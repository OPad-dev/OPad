#include "module_watch.h"

#include <stddef.h>

void module_watch_init(module_watch_t *w, uint8_t boot_module)
{
    if (w == NULL) {
        return;
    }
    *w = (module_watch_t){.boot = boot_module, .last = boot_module};
}

bool module_watch_fast(module_watch_t *w, bool disagrees)
{
    if (!disagrees) {
        w->fast_misses = 0;
        return false;
    }
    if (w->fast_misses >= MODULE_WATCH_FAST_READS) {
        return false;  // passed on already
    }
    return ++w->fast_misses == MODULE_WATCH_FAST_READS;
}

module_watch_action_t module_watch_step(module_watch_t *w, uint8_t module_now)
{
    bool changed = module_now != w->last;
    w->last = module_now;

    if (module_now != w->boot) {
        w->matches = 0;
        if (!w->keys_off) {
            w->keys_off = true;
            return MODULE_WATCH_KEYS_OFF;
        }
        return changed ? MODULE_WATCH_CHANGED : MODULE_WATCH_KEEP;
    }
    if (w->keys_off) {
        if (++w->matches >= MODULE_WATCH_RESTORE_READS) {
            w->keys_off = false;
            w->matches = 0;
            w->fast_misses = 0;  // counting starts over with the keys back on
            return MODULE_WATCH_KEYS_ON;
        }
        return changed ? MODULE_WATCH_CHANGED : MODULE_WATCH_KEEP;
    }
    return MODULE_WATCH_KEEP;
}
