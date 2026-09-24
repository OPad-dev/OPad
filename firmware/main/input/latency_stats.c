#include "latency_stats.h"
#include "esp_attr.h"
#include <stdatomic.h>
#include <string.h>

// 10 us buckets up to 5 ms; slower samples land in the last bucket (max_us is exact)
#define BUCKET_US    10
#define BUCKET_COUNT 500

static atomic_uint_least32_t s_buckets[BUCKET_COUNT];
static atomic_uint_least32_t s_max_us;
static atomic_uint_least32_t s_deferred;
static atomic_uint_least32_t s_outlier_count;
static atomic_uint_least32_t s_outlier_max_us;

void IRAM_ATTR latency_stats_record(uint32_t latency_us)
{
    uint32_t idx = latency_us / BUCKET_US;
    if (idx >= BUCKET_COUNT) {
        idx = BUCKET_COUNT - 1;
    }
    atomic_fetch_add_explicit(&s_buckets[idx], 1, memory_order_relaxed);
    if (latency_us > atomic_load_explicit(&s_max_us, memory_order_relaxed)) {
        atomic_store_explicit(&s_max_us, latency_us, memory_order_relaxed);
    }
    if (latency_us > 1000) {
        atomic_fetch_add_explicit(&s_outlier_count, 1, memory_order_relaxed);
        uint32_t cur_max = atomic_load_explicit(&s_outlier_max_us, memory_order_relaxed);
        while (latency_us > cur_max && !atomic_compare_exchange_weak_explicit(&s_outlier_max_us, &cur_max, latency_us, memory_order_relaxed, memory_order_relaxed)) {
        }
    }
}

void IRAM_ATTR latency_stats_record_deferred(void)
{
    atomic_fetch_add_explicit(&s_deferred, 1, memory_order_relaxed);
}

static uint32_t percentile_us(const uint32_t *buckets, uint32_t total, uint32_t per_mille)
{
    // Smallest bucket upper edge covering the requested fraction of samples
    uint64_t target = ((uint64_t)total * per_mille + 999) / 1000;
    uint64_t seen = 0;
    for (int i = 0; i < BUCKET_COUNT; i++) {
        seen += buckets[i];
        if (seen >= target) {
            return (uint32_t)((i + 1) * BUCKET_US);
        }
    }
    return BUCKET_COUNT * BUCKET_US;
}

void latency_stats_get(latency_stats_t *out)
{
    static uint32_t snapshot[BUCKET_COUNT];
    uint32_t total = 0;
    for (int i = 0; i < BUCKET_COUNT; i++) {
        snapshot[i] = atomic_load_explicit(&s_buckets[i], memory_order_relaxed);
        total += snapshot[i];
    }

    memset(out, 0, sizeof(*out));
    out->samples = total;
    out->max_us = atomic_load_explicit(&s_max_us, memory_order_relaxed);
    out->deferred_reports = atomic_load_explicit(&s_deferred, memory_order_relaxed);
    if (total > 0) {
        out->p50_us = percentile_us(snapshot, total, 500);
        out->p99_us = percentile_us(snapshot, total, 990);
        out->p999_us = percentile_us(snapshot, total, 999);
    }
}

void latency_stats_reset(void)
{
    for (int i = 0; i < BUCKET_COUNT; i++) {
        atomic_store_explicit(&s_buckets[i], 0, memory_order_relaxed);
    }
    atomic_store_explicit(&s_max_us, 0, memory_order_relaxed);
    atomic_store_explicit(&s_deferred, 0, memory_order_relaxed);
    atomic_store_explicit(&s_outlier_count, 0, memory_order_relaxed);
    atomic_store_explicit(&s_outlier_max_us, 0, memory_order_relaxed);
}

bool latency_stats_drain_outlier(uint32_t *out_max_us, uint32_t *out_count)
{
    uint32_t count = atomic_exchange_explicit(&s_outlier_count, 0, memory_order_relaxed);
    if (count == 0) {
        return false;
    }
    if (out_count) *out_count = count;
    if (out_max_us) *out_max_us = atomic_exchange_explicit(&s_outlier_max_us, 0, memory_order_relaxed);
    return true;
}
