#include "usb_report_barrier.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "tusb.h"
#include "device/usbd_pvt.h"
#include <stdatomic.h>
#include <stdint.h>

static atomic_uint s_requested;
static atomic_uint s_completed;

static void report_barrier_ack(void *arg)
{
    // Runs on the same task as HID completion, behind any running callback.
    atomic_store_explicit(&s_completed, (unsigned)(uintptr_t)arg, memory_order_release);
}

esp_err_t usb_report_barrier_wait(uint32_t timeout_ms)
{
    if (!tud_inited()) return ESP_ERR_INVALID_STATE;
    unsigned ticket = atomic_fetch_add_explicit(&s_requested, 1, memory_order_relaxed) + 1;
    usbd_defer_func(report_barrier_ack, (void *)(uintptr_t)ticket, false);
    TickType_t start = xTaskGetTickCount();
    while (atomic_load_explicit(&s_completed, memory_order_acquire) != ticket) {
        if ((TickType_t)(xTaskGetTickCount() - start) >= pdMS_TO_TICKS(timeout_ms)) {
            return ESP_ERR_TIMEOUT;
        }
        vTaskDelay(1);
    }
    return ESP_OK;
}
