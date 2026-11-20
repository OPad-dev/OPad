#pragma once

#include <stdbool.h>

/*
 * The NVS namespaces the firmware stores its state in.
 *
 * Firmware up to 1.0.0-rc stored them under osupad* names. This firmware
 * stores them under opad*: at boot, nvs_ns_migrate() copies each old
 * namespace that has not been copied yet into its new name, key by key, and
 * writes a marker key last. The old namespace is never changed, so an OTA
 * rollback to older firmware finds its data as it was at the upgrade.
 *
 * A namespace whose copy fails (for example, NVS full) keeps its old name
 * for this boot and is tried again at the next one; nothing is lost.
 */
typedef enum {
    NVS_NS_COUNTERS = 0,  // lifetime counters, generation       (was "osupad")
    NVS_NS_CONFIG,        // device config, owner, HE settings   (was "osupad_cfg")
    NVS_NS_CAL,           // HE V1 calibration                   (was "osupad_cal")
    NVS_NS_CAL3,          // HE V3 calibration                   (was "osupad_cal3")
    NVS_NS_UI,            // screen layouts                      (was "osupad_ui")
    NVS_NS_COUNT
} nvs_ns_id_t;

/* The key written last into a new namespace once its copy is complete */
#define NVS_NS_MIGRATED_KEY "_migrated"

/**
 * Copies every old namespace into its new name if that has not happened yet.
 * Call once at boot, after nvs_flash_init() succeeded and before any module
 * opens its namespace. Never erases anything but a half-finished copy.
 */
void nvs_ns_migrate(void);

/** The namespace to open for id: the new name, or the old one if it could not be copied */
const char *nvs_ns_name(nvs_ns_id_t id);

/** The name before the rename, for tests and diagnostics */
const char *nvs_ns_legacy_name(nvs_ns_id_t id);
