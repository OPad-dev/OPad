#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include "usb/usb_report_barrier.h"
#include "freertos/task.h"
#include "device/usbd_pvt.h"

// Real barrier implementation, with an explicit FIFO/callback scheduler at
// the TinyUSB boundary. This is not a USB transfer or RTOS timing emulation.
typedef struct { osal_task_func_t func; void *arg; } event_t;
static event_t events[16];
static unsigned head, tail;
static TickType_t now;
static bool initialized = true;
static bool run_callbacks = true;
static bool old_callback_finished;

bool tud_inited(void) { return initialized; }
TickType_t xTaskGetTickCount(void) { return now; }
void usbd_defer_func(osal_task_func_t func, void *param, bool in_isr)
{
    assert(!in_isr && tail < 16);
    events[tail++] = (event_t){func, param};
}
void vTaskDelay(TickType_t ticks)
{
    now += ticks;
    if (run_callbacks && head < tail) {
        event_t e = events[head++];
        e.func(e.arg);
    }
}
static void finish_old_callback(void *arg)
{
    (void)arg;
    old_callback_finished = true;
}

int main(void)
{
    initialized = false;
    assert(usb_report_barrier_wait(5) == ESP_ERR_INVALID_STATE);
    assert(tail == 0);
    initialized = true;

    usbd_defer_func(finish_old_callback, NULL, false);
    assert(usb_report_barrier_wait(5) == ESP_OK);
    assert(old_callback_finished && head == tail && now == 2);

    run_callbacks = false;
    assert(usb_report_barrier_wait(3) == ESP_ERR_TIMEOUT);
    unsigned old_head = head;
    run_callbacks = true;
    // The first tick runs the timed-out ack; it cannot satisfy this retry.
    TickType_t before = now;
    assert(usb_report_barrier_wait(5) == ESP_OK);
    assert(head == old_head + 2 && now - before == 2);

    // An immediate timeout still leaves a safe, value-only queued ticket.
    assert(usb_report_barrier_wait(0) == ESP_ERR_TIMEOUT);
    assert(usb_report_barrier_wait(1) == ESP_ERR_TIMEOUT);
    assert(usb_report_barrier_wait(5) == ESP_OK);

    now = UINT_MAX - 1;
    run_callbacks = false;
    assert(usb_report_barrier_wait(3) == ESP_ERR_TIMEOUT);
    assert(now == 1); // tick subtraction survives wrap
    run_callbacks = true;
    assert(usb_report_barrier_wait(5) == ESP_OK);
    puts("USB report barrier waits for FIFO callbacks and rejects stale acknowledgements.");
    return 0;
}
