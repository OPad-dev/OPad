#pragma once
#include <stdbool.h>
typedef void (*osal_task_func_t)(void *);
void usbd_defer_func(osal_task_func_t func, void *param, bool in_isr);
