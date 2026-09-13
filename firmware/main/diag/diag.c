#include "diag.h"
#include <string.h>

#ifdef ESP_PLATFORM
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
static portMUX_TYPE s_mux = portMUX_INITIALIZER_UNLOCKED;
#define DIAG_ENTER_CRITICAL() taskENTER_CRITICAL(&s_mux)
#define DIAG_EXIT_CRITICAL()  taskEXIT_CRITICAL(&s_mux)
#define DIAG_GET_TIME_MS()    ((uint32_t)(esp_timer_get_time() / 1000))
#else
#define DIAG_ENTER_CRITICAL() ((void)0)
#define DIAG_EXIT_CRITICAL()  ((void)0)
static uint32_t s_host_time_ms = 1000;
#define DIAG_GET_TIME_MS()    (s_host_time_ms++)
#endif

static diag_entry_t s_entries[DIAG_RING_SIZE];
static size_t s_head = 0;
static size_t s_tail = 0;
static size_t s_count = 0;
static uint32_t s_dropped = 0;

void diag_init(void)
{
    DIAG_ENTER_CRITICAL();
    s_head = 0;
    s_tail = 0;
    s_count = 0;
    s_dropped = 0;
    memset(s_entries, 0, sizeof(s_entries));
    DIAG_EXIT_CRITICAL();
}

void diag_record(uint16_t event_id, uint8_t level, uint32_t arg0, uint32_t arg1)
{
    uint32_t now_ms = DIAG_GET_TIME_MS();

    DIAG_ENTER_CRITICAL();
    if (s_count >= DIAG_RING_SIZE) {
        // Drop the oldest entry
        s_tail = (s_tail + 1) % DIAG_RING_SIZE;
        s_dropped++;
    } else {
        s_count++;
    }

    s_entries[s_head].timestamp_ms = now_ms;
    s_entries[s_head].event_id = event_id;
    s_entries[s_head].level = level;
    s_entries[s_head].arg0 = arg0;
    s_entries[s_head].arg1 = arg1;
    s_head = (s_head + 1) % DIAG_RING_SIZE;
    DIAG_EXIT_CRITICAL();
}

size_t diag_drain(diag_entry_t *out_entries, size_t max_entries, uint32_t *out_dropped)
{
    if (!out_entries || max_entries == 0) {
        return 0;
    }

    DIAG_ENTER_CRITICAL();
    if (out_dropped) {
        *out_dropped = s_dropped;
        s_dropped = 0;
    }

    size_t num = (s_count < max_entries) ? s_count : max_entries;
    for (size_t i = 0; i < num; i++) {
        out_entries[i] = s_entries[s_tail];
        s_tail = (s_tail + 1) % DIAG_RING_SIZE;
    }
    s_count -= num;
    DIAG_EXIT_CRITICAL();

    return num;
}

size_t diag_available(void)
{
    DIAG_ENTER_CRITICAL();
    size_t c = s_count;
    DIAG_EXIT_CRITICAL();
    return c;
}
