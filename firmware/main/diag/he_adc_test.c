#include "he_adc_test.h"
#include "input/ads7028.h"
#include "input/ads7028_regs.h"
#include "esp_log.h"
#include "esp_rom_sys.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include <math.h>
#include <stdio.h>
#include <string.h>

static const char *TAG = "he_adc_test";

// Registers and sensors run at 4 MHz: a 24-bit frame in 6 us, far below what
// the 80 mm FFC and its 33 ohm series resistors allow
#define WORK_SCLK_HZ   4000000
#define PATTERN_FRAMES 500

static const uint32_t SPEEDS_HZ[HE_ADC_TEST_SPEEDS] = {
    1000000, 4000000, 10000000, 20000000, 26000000, 40000000,
};

// Key 1, 2, 3 (hardware/verify/golden/he_v3.toml on hw/redesign: U4 AIN5/4/3)
static const uint8_t KEY_AIN[HE_ADC_TEST_KEYS] = {5, 4, 3};

static void fail(he_adc_test_t *r, const char *step, esp_err_t err)
{
    snprintf(r->message, sizeof(r->message), "%s: %s", step, esp_err_to_name(err));
}

static esp_err_t read_status(int *out)
{
    uint8_t v = 0;
    esp_err_t err = ads7028_read_reg(ADS7028_REG_SYSTEM_STATUS, &v);
    *out = err == ESP_OK ? v : -1;
    return err;
}

static esp_err_t register_walk(he_adc_test_t *r)
{
    uint8_t patterns[4 + 16] = {0x00, 0xFF, 0x55, 0xAA};
    for (int i = 0; i < 8; i++) {
        patterns[4 + i] = (uint8_t)(1u << i);
        patterns[12 + i] = (uint8_t)~(1u << i);
    }
    for (size_t i = 0; i < sizeof(patterns); i++) {
        uint8_t back = 0;
        esp_err_t err = ads7028_write_reg(ADS7028_REG_AUTO_SEQ_CH_SEL, patterns[i]);
        if (err == ESP_OK) {
            err = ads7028_read_reg(ADS7028_REG_AUTO_SEQ_CH_SEL, &back);
        }
        if (err != ESP_OK) {
            return err;
        }
        r->walk_writes++;
        if (back != patterns[i]) {
            r->walk_mismatches++;
            r->walk_wrong_bits |= (uint8_t)(back ^ patterns[i]);
        }
    }
    return ads7028_write_reg(ADS7028_REG_AUTO_SEQ_CH_SEL, 0);
}

static esp_err_t calibrate(he_adc_test_t *r)
{
    esp_err_t err = ads7028_write_reg(ADS7028_REG_GENERAL_CFG, ADS7028_GENERAL_CAL);
    if (err != ESP_OK) {
        return err;
    }
    for (int i = 0; i < 20; i++) {
        uint8_t cfg = 0xFF;
        esp_rom_delay_us(500);
        err = ads7028_read_reg(ADS7028_REG_GENERAL_CFG, &cfg);
        if (err != ESP_OK) {
            return err;
        }
        if ((cfg & ADS7028_GENERAL_CAL) == 0) {
            r->cal_done = true;
            return ESP_OK;
        }
    }
    return ESP_OK;  // still set after 10 ms: cal_done stays false
}

static esp_err_t speed_sweep(he_adc_test_t *r)
{
    static const uint8_t nop[3] = {0};
    esp_err_t err = ads7028_write_reg(ADS7028_REG_DATA_CFG, ADS7028_DATA_FIX_PAT);
    if (err != ESP_OK) {
        return err;
    }
    for (int s = 0; s < HE_ADC_TEST_SPEEDS; s++) {
        he_adc_speed_t *sp = &r->speeds[r->speed_count++];
        *sp = (he_adc_speed_t){.sclk_hz = SPEEDS_HZ[s]};
        if (ads7028_set_clock(SPEEDS_HZ[s]) != ESP_OK) {
            continue;  // faster than the GPIO matrix allows: frames stays 0
        }
        uint8_t rx[3];
        for (int i = 0; i < 2; i++) {
            ads7028_transfer(nop, rx);  // the frames before the pattern
        }
        for (int i = 0; i < PATTERN_FRAMES; i++) {
            if (ads7028_transfer(nop, rx) != ESP_OK) {
                sp->errors++;
            } else if (ads7028_data12(rx) != ADS7028_FIXED_PATTERN) {
                sp->errors++;
            }
            sp->frames++;
        }
    }
    err = ads7028_set_clock(WORK_SCLK_HZ);
    if (err == ESP_OK) {
        err = ads7028_write_reg(ADS7028_REG_DATA_CFG, 0);
    }
    return err;
}

/*
 * Manual mode (8.4.2): the CHANNEL_SEL write is decoded at CS rising, which
 * also samples the old channel; the next frame samples the new one and the
 * frame after it returns it. So two frames are thrown away, then every frame
 * returns the conversion its previous CS rising edge started.
 */
static esp_err_t read_channel(uint8_t ain, he_adc_channel_t *c)
{
    static const uint8_t nop[3] = {0};
    esp_err_t err = ads7028_write_reg(ADS7028_REG_CHANNEL_SEL, ain);
    if (err != ESP_OK) {
        return err;
    }
    uint8_t rx[3];
    for (int i = 0; i < 2; i++) {
        if ((err = ads7028_transfer(nop, rx)) != ESP_OK) {
            return err;
        }
    }
    *c = (he_adc_channel_t){.ain = ain, .min = 0xFFFF};
    double sum = 0;
    double sum_sq = 0;
    for (int i = 0; i < HE_ADC_TEST_SAMPLES; i++) {
        if ((err = ads7028_transfer(nop, rx)) != ESP_OK) {
            return err;
        }
        uint16_t v = ads7028_data12(rx);
        c->min = v < c->min ? v : c->min;
        c->max = v > c->max ? v : c->max;
        sum += v;
        sum_sq += (double)v * v;
        c->samples++;
    }
    double mean = sum / c->samples;
    double var = sum_sq / c->samples - mean * mean;
    c->mean_x16 = (uint32_t)lround(mean * 16.0);
    c->noise_x100 = (uint32_t)lround(sqrt(var > 0 ? var : 0) * 100.0);
    return ESP_OK;
}

void he_adc_test_run(bool cable_reversed, he_adc_test_t *out)
{
    he_adc_test_t r = {
        .cable_reversed = cable_reversed,
        .sclk_hz = WORK_SCLK_HZ,
        .status_after_reset = -1,
        .status_after_clear = -1,
        .status_end = -1,
    };
    ads7028_pins_t pins;
    ads7028_carrier_pins(cable_reversed, &pins);
    esp_err_t err = ads7028_open(&pins, WORK_SCLK_HZ);
    if (err != ESP_OK) {
        fail(&r, "SPI", err);
        *out = r;
        return;
    }
    r.ran = true;

    do {
        if ((err = ads7028_write_reg(ADS7028_REG_GENERAL_CFG, ADS7028_GENERAL_RST)) != ESP_OK) {
            fail(&r, "reset", err);
            break;
        }
        vTaskDelay(pdMS_TO_TICKS(ADS7028_RESET_MS + 1));
        if ((err = read_status(&r.status_after_reset)) != ESP_OK) {
            fail(&r, "status", err);
            break;
        }
        if ((err = ads7028_write_reg(ADS7028_REG_SYSTEM_STATUS, ADS7028_STATUS_BOR)) != ESP_OK ||
            (err = read_status(&r.status_after_clear)) != ESP_OK) {
            fail(&r, "clear BOR", err);
            break;
        }
        if ((err = register_walk(&r)) != ESP_OK) {
            fail(&r, "register walk", err);
            break;
        }
        if ((err = calibrate(&r)) != ESP_OK) {
            fail(&r, "calibration", err);
            break;
        }
        if ((err = speed_sweep(&r)) != ESP_OK) {
            fail(&r, "speed sweep", err);
            break;
        }
        for (int k = 0; k < HE_ADC_TEST_KEYS; k++) {
            if ((err = read_channel(KEY_AIN[k], &r.channels[k])) != ESP_OK) {
                fail(&r, "sensors", err);
                break;
            }
            r.channel_count++;
        }
        if (err != ESP_OK) {
            break;
        }
        ads7028_write_reg(ADS7028_REG_CHANNEL_SEL, 0);
        read_status(&r.status_end);
    } while (0);

    ads7028_close();

    ESP_LOGI(TAG, "%s cable, status 0x%02x/0x%02x/0x%02x, walk %lu/%lu wrong 0x%02x, cal %d, "
             "keys %u/%u/%u (noise x100 %lu/%lu/%lu) %s",
             cable_reversed ? "reversed" : "normal", r.status_after_reset & 0xFF,
             r.status_after_clear & 0xFF, r.status_end & 0xFF,
             (unsigned long)r.walk_mismatches, (unsigned long)r.walk_writes, r.walk_wrong_bits,
             r.cal_done, (unsigned)(r.channels[0].mean_x16 / 16), (unsigned)(r.channels[1].mean_x16 / 16),
             (unsigned)(r.channels[2].mean_x16 / 16), (unsigned long)r.channels[0].noise_x100,
             (unsigned long)r.channels[1].noise_x100, (unsigned long)r.channels[2].noise_x100,
             r.message);
    *out = r;
}
