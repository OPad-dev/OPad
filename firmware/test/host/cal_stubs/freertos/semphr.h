#pragma once
#include "FreeRTOS.h"
typedef pthread_mutex_t StaticSemaphore_t;
typedef pthread_mutex_t *SemaphoreHandle_t;
static inline SemaphoreHandle_t xSemaphoreCreateMutexStatic(StaticSemaphore_t *buf)
{
    pthread_mutex_init(buf, NULL);
    return buf;
}
static inline int xSemaphoreTake(SemaphoreHandle_t mutex, uint32_t ticks)
{
    (void)ticks;
    return pthread_mutex_lock(mutex) == 0;
}
static inline int xSemaphoreGive(SemaphoreHandle_t mutex)
{
    return pthread_mutex_unlock(mutex) == 0;
}
