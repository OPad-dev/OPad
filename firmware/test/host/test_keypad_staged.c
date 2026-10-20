#include <assert.h>
#include <stdbool.h>
#include <stdio.h>

/*
 * Verification test for keypad staged config rule:
 * Keycode and pin changes must wait until all keys are released (both MX switches
 * and Hall Effect sensors via key_event_is_down).
 */

static inline bool keypad_can_apply_staged_config(bool mx_state0, bool mx_state1,
                                                  bool mx_rep0, bool mx_rep1,
                                                  bool he_down0, bool he_down1)
{
    return !mx_state0 && !mx_state1 &&
           !mx_rep0 && !mx_rep1 &&
           !he_down0 && !he_down1;
}

static void test_keypad_staging_rule(void)
{
    // Both MX and HE released: can apply
    assert(keypad_can_apply_staged_config(false, false, false, false, false, false));

    // MX held: blocked
    assert(!keypad_can_apply_staged_config(true, false, false, false, false, false));
    assert(!keypad_can_apply_staged_config(false, true, false, false, false, false));
    assert(!keypad_can_apply_staged_config(false, false, true, false, false, false));
    assert(!keypad_can_apply_staged_config(false, false, false, true, false, false));

    // HE held (on HE pad, MX states are false): blocked (prevents mid-hold phantom keycodes)
    assert(!keypad_can_apply_staged_config(false, false, false, false, true, false));
    assert(!keypad_can_apply_staged_config(false, false, false, false, false, true));
    assert(!keypad_can_apply_staged_config(false, false, false, false, true, true));

    // Both MX and HE held: blocked
    assert(!keypad_can_apply_staged_config(true, false, false, false, true, false));

    printf("✓ test_keypad_staging_rule passed\n");
}

int main(void)
{
    printf("=== Running Keypad Config Staging Host Tests ===\n");
    test_keypad_staging_rule();
    printf("All keypad config staging tests passed successfully!\n");
    return 0;
}
