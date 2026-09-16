#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Length of a host install identity (a UUIDv4's bytes, §W3-1). */
#define OWNER_ID_LEN 16

/**
 * @brief Runtime state as far as ownership is concerned.
 *
 * Claiming writes NVS, so it obeys P1-3 exactly like every other write:
 * IDLE only, never during PLAYING or COOLDOWN.
 */
typedef enum {
    OWNER_STATE_IDLE = 0,
    OWNER_STATE_ACTIVE = 1,  // PLAYING or COOLDOWN
} owner_runtime_state_t;

typedef enum {
    /** Record the requested owner. */
    OWNER_CLAIM_APPLY,
    /** Already this owner; writing again would burn a flash cycle per connect. */
    OWNER_CLAIM_ALREADY_OWNED,
    /** P1-3: not while a map is running. */
    OWNER_CLAIM_REJECT_ACTIVE,
    /** Missing, or all zero. */
    OWNER_CLAIM_REJECT_INVALID,
} owner_claim_action_t;

/** @brief True when no host owns this pad. An all-zero id is "unclaimed". */
bool owner_is_unclaimed(const uint8_t *owner);

/** @brief Constant-length compare of two owner ids. */
bool owner_equals(const uint8_t *a, const uint8_t *b);

/**
 * @brief Decides what a ClaimOwnership request should do (§W3-2).
 *
 * Pure, so the host tests cover it without ESP-IDF, in the same way P0-2 split
 * the debounce decision out of the ISR.
 *
 * Deliberately *not* a policy engine. The firmware does not arbitrate between
 * hosts: the host prompts the user and only then sends a claim, so a claim
 * from a different install is applied rather than questioned. Two things it
 * does refuse — a write while a map is running, and an all-zero id, because
 * unclaiming over the wire would be an unpair command, and §W3-4 makes a full
 * reflash the only way to unbind a pad.
 */
owner_claim_action_t owner_claim_decide(const uint8_t *current,
                                        const uint8_t *requested,
                                        owner_runtime_state_t state);

#ifdef __cplusplus
}
#endif
