/*
 * Analyse a raw Hall Effect trace (CONFIG_OSUPAD_HE_TRACE, recorded with
 * scripts/he_trace.py) of keys that do NOT move: at rest, or held still.
 * For OPEN A/D in docs/specs/v2-hall-effect-decisions.md.
 *
 *   he_replay TRACE.bin
 *
 * Prints whether the stream is complete (sequence gaps, frames the pad
 * dropped, the pad's last health counters), then per key and per estimator
 * (newest conversion of a frame, mean of the newest two, mean of all eight):
 *   - spread: sd, peak to peak overall, and the worst peak to peak within
 *     5 ms and 1 s windows
 *   - retrace: the largest fall below the running maximum and rise above the
 *     running minimum. A still key held PRESSED releases falsely once its
 *     reading falls rt_release below its peak, so rapid trigger needs a
 *     sensitivity above the retrace of the longest hold it must survive.
 *   - false events: the real engine (firmware/main/input/he_rt.c) run over
 *     the trace at candidate sensitivities, the key held PRESSED (peak
 *     tracking) and RT_RELEASED (trough tracking). Every event is false,
 *     since the key does not move.
 *
 * Built by scripts/he_trace.py; nothing here runs on the pad.
 */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "input/he_rt.h"

#define CONVS 16
#define REC_BYTES (6 + CONVS * 2)
#define STATS_BYTES (2 + 10 * 4)
#define WIN_5MS 20      // frames
#define WIN_1S 4000

enum { EST_NEWEST, EST_AVG2, EST_AVG8, EST_COUNT };
static const char *EST_NAME[EST_COUNT] = {"newest", "avg2", "avg8"};

static const int32_t SENS[] = {4, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 40, 48, 64};
#define SENS_COUNT ((int)(sizeof(SENS) / sizeof(SENS[0])))

typedef struct {
    int32_t *v;
    size_t n, cap;
} series_t;

static void push(series_t *s, int32_t x)
{
    if (s->n == s->cap) {
        s->cap = s->cap ? s->cap * 2 : 65536;
        s->v = realloc(s->v, s->cap * sizeof(int32_t));
        if (!s->v) {
            fprintf(stderr, "out of memory\n");
            exit(1);
        }
    }
    s->v[s->n++] = x;
}

static uint32_t rd16(const uint8_t *p)
{
    return p[0] | ((uint32_t)p[1] << 8);
}

static uint32_t rd32(const uint8_t *p)
{
    return rd16(p) | (rd16(p + 2) << 16);
}

static int32_t avg_round(int32_t sum, int32_t n)
{
    return (sum + n / 2) / n;
}

static int count_events(const series_t *s, int32_t sens, bool start_pressed)
{
    int32_t lo = s->v[0], hi = s->v[0];
    for (size_t i = 1; i < s->n; i++) {
        lo = s->v[i] < lo ? s->v[i] : lo;
        hi = s->v[i] > hi ? s->v[i] : hi;
    }
    // Every reading inside the rapid-trigger region, so only sensitivity matters
    he_rt_params_t p = {
        .actuation = lo, .release = lo - 1, .top_reset = lo - 1,
        .rt_press = sens, .rt_release = sens, .peak_cap = INT32_MAX,
        .rapid_trigger = true,
    };
    he_rt_key_t k;
    he_rt_init(&k, &p);
    he_rt_step(&k, lo - 1);                // seen up: armed
    if (start_pressed) {
        he_rt_step(&k, s->v[0]);           // DOWN at the first reading, peak there
    } else {
        he_rt_step(&k, hi + sens + 1);     // DOWN above everything...
        he_rt_step(&k, s->v[0]);           // ...UP to RT_RELEASED, trough at the first reading
    }
    int events = 0;
    for (size_t i = 1; i < s->n; i++) {
        if (he_rt_step(&k, s->v[i]) != HE_RT_NONE) {
            events++;
        }
    }
    return events;
}

static int32_t window_pp(const series_t *s, size_t win)
{
    int32_t worst = 0;
    for (size_t a = 0; a + win <= s->n; a += win) {
        int32_t lo = s->v[a], hi = s->v[a];
        for (size_t i = a; i < a + win; i++) {
            lo = s->v[i] < lo ? s->v[i] : lo;
            hi = s->v[i] > hi ? s->v[i] : hi;
        }
        worst = hi - lo > worst ? hi - lo : worst;
    }
    return worst;
}

static void analyse(int key, int est, const series_t *s)
{
    if (s->n < 2) {
        printf("  key %d %-6s: no readings\n", key + 1, EST_NAME[est]);
        return;
    }
    double sum = 0, sq = 0;
    int32_t lo = s->v[0], hi = s->v[0];
    int32_t run_max = s->v[0], run_min = s->v[0], fall = 0, rise = 0;
    for (size_t i = 0; i < s->n; i++) {
        int32_t x = s->v[i];
        sum += x;
        sq += (double)x * x;
        lo = x < lo ? x : lo;
        hi = x > hi ? x : hi;
        run_max = x > run_max ? x : run_max;
        run_min = x < run_min ? x : run_min;
        fall = run_max - x > fall ? run_max - x : fall;
        rise = x - run_min > rise ? x - run_min : rise;
    }
    double mean = sum / s->n;
    double sd = sqrt(sq / s->n - mean * mean);
    printf("  key %d %-6s: %zu frames, mean %.1f, sd %.2f, p-p %d, worst p-p 5 ms %d / 1 s %d, "
           "retrace fall %d rise %d\n",
           key + 1, EST_NAME[est], s->n, mean, sd, hi - lo, window_pp(s, WIN_5MS), window_pp(s, WIN_1S),
           fall, rise);
    printf("    false events (held PRESSED / RT_RELEASED) by sensitivity:");
    for (int i = 0; i < SENS_COUNT; i++) {
        printf(" %d:%d/%d", SENS[i], count_events(s, SENS[i], true), count_events(s, SENS[i], false));
    }
    printf("\n");
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: %s TRACE.bin\n", argv[0]);
        return 2;
    }
    FILE *f = fopen(argv[1], "rb");
    if (!f) {
        perror(argv[1]);
        return 1;
    }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t *buf = malloc((size_t)size + 1);
    if (!buf || fread(buf, 1, (size_t)size, f) != (size_t)size) {
        fprintf(stderr, "cannot read %s\n", argv[1]);
        return 1;
    }
    fclose(f);

    series_t est[2][EST_COUNT] = {{{0}}};
    size_t frames = 0, lost = 0, skipped_bytes = 0, bad_convs = 0;
    uint32_t dropped = 0, stats[10] = {0};
    bool have_stats = false, have_seq = false;
    uint16_t last_seq = 0;

    for (long i = 0; i + 1 < size;) {
        if (buf[i] == 0xA5 && buf[i + 1] == 0x5A && i + REC_BYTES <= size) {
            const uint8_t *r = buf + i;
            uint16_t seq = (uint16_t)rd16(r + 2);
            if (have_seq) {
                lost += (uint16_t)(seq - last_seq - 1);
            }
            have_seq = true;
            last_seq = seq;
            dropped = rd16(r + 4);
            int32_t vals[2][CONVS / 2];
            int n[2] = {0, 0};
            for (int c = 0; c < CONVS; c++) {
                uint32_t v = rd16(r + 6 + c * 2);
                if (v & 0x4000) {
                    bad_convs++;
                    continue;
                }
                int key = (v & 0x8000) ? 1 : 0;
                if (n[key] < CONVS / 2) {
                    vals[key][n[key]++] = (int32_t)(v & 0xFFF);
                }
            }
            for (int k = 0; k < 2; k++) {
                if (n[k] == 0) {
                    continue;
                }
                int32_t last = vals[k][n[k] - 1];
                int32_t sum = 0;
                for (int j = 0; j < n[k]; j++) {
                    sum += vals[k][j];
                }
                push(&est[k][EST_NEWEST], last);
                push(&est[k][EST_AVG2], n[k] >= 2 ? avg_round(last + vals[k][n[k] - 2], 2) : last);
                push(&est[k][EST_AVG8], avg_round(sum, n[k]));
            }
            frames++;
            i += REC_BYTES;
        } else if (buf[i] == 0xA5 && buf[i + 1] == 0x5B && i + STATS_BYTES <= size) {
            for (int s = 0; s < 10; s++) {
                stats[s] = rd32(buf + i + 2 + s * 4);
            }
            have_stats = true;
            i += STATS_BYTES;
        } else {
            skipped_bytes++;
            i++;
        }
    }

    printf("%zu frames (%.1f s at 4000/s), %zu lost on the way to the host, %u dropped on the pad "
           "(16-bit), %zu bytes skipped resyncing, %zu conversions not a valid key reading\n",
           frames, frames / 4000.0, lost, dropped, skipped_bytes, bad_convs);
    if (have_stats) {
        printf("pad: frames %u, late %u, max gap %u us, foreign %u, railed %u, key faults %u, stalls %u, "
               "restarts %u, frame interrupt max %u cycles (%.1f us)\n",
               stats[0], stats[1], stats[2], stats[3], stats[4], stats[5], stats[6], stats[7], stats[8],
               stats[8] / 240.0);
    }
    if (lost || dropped) {
        printf("WARNING: incomplete stream; noise figures still hold, but retrace across a gap is not continuous\n");
    }
    for (int k = 0; k < 2; k++) {
        for (int e = 0; e < EST_COUNT; e++) {
            analyse(k, e, &est[k][e]);
        }
    }
    return 0;
}
