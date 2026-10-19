#pragma once

/*
 * Input module swapped while powered. The key input is chosen once, at boot,
 * for the module found then (board_detect_module). A Hall Effect module
 * plugged into a pad that booted with MX puts analog sensor outputs (~1.8 V)
 * on digital key pins: they chatter, and every edge became a key press sent
 * to the host (tens of thousands of them).
 *
 * The runtime polls the module ID; this decides what the readings mean. Pure
 * C, host-tested (firmware/test/host/test_module_watch.c). Module values are
 * board_module_type_t's (board.h).
 *
 * The first reading that is not the boot module turns the keys off: pulling
 * the old module out reads as "none" well before a new one is in, so the keys
 * are off before anything can chatter. They come back only after
 * MODULE_WATCH_RESTORE_READS readings of the boot module in a row (the same
 * module seated again). A different module needs a replug, as at boot.
 */

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define MODULE_WATCH_RESTORE_READS 3

typedef enum {
    MODULE_WATCH_KEEP = 0,      // nothing to do
    MODULE_WATCH_KEYS_OFF = 1,  // the module changed: keys off
    MODULE_WATCH_KEYS_ON = 2,   // the boot module is back: keys on
    MODULE_WATCH_CHANGED = 3,   // keys stay off, but what is plugged in changed again
} module_watch_action_t;

typedef struct {
    uint8_t boot;       // module at boot
    uint8_t last;       // last reading
    bool keys_off;
    uint8_t matches;    // boot-module readings in a row while off
} module_watch_t;

void module_watch_init(module_watch_t *w, uint8_t boot_module);

module_watch_action_t module_watch_step(module_watch_t *w, uint8_t module_now);

#ifdef __cplusplus
}
#endif
