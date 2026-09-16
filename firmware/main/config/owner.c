#include "owner.h"
#include <string.h>

bool owner_is_unclaimed(const uint8_t *owner)
{
    if (!owner) {
        return true;
    }
    for (int i = 0; i < OWNER_ID_LEN; i++) {
        if (owner[i] != 0) {
            return false;
        }
    }
    return true;
}

bool owner_equals(const uint8_t *a, const uint8_t *b)
{
    if (!a || !b) {
        return false;
    }
    return memcmp(a, b, OWNER_ID_LEN) == 0;
}

owner_claim_action_t owner_claim_decide(const uint8_t *current,
                                        const uint8_t *requested,
                                        owner_runtime_state_t state)
{
    // An all-zero claim would be an unpair command; §W3-4 makes reflashing the
    // only way to unbind, so there is deliberately no wire path to it.
    if (!requested || owner_is_unclaimed(requested)) {
        return OWNER_CLAIM_REJECT_INVALID;
    }

    // Checked before the state guard: re-sending the same owner is a no-op,
    // and a no-op need not wait for the map to end.
    if (owner_equals(current, requested)) {
        return OWNER_CLAIM_ALREADY_OWNED;
    }

    if (state != OWNER_STATE_IDLE) {
        return OWNER_CLAIM_REJECT_ACTIVE;
    }

    return OWNER_CLAIM_APPLY;
}
