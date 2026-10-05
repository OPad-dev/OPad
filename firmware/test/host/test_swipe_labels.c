// Host tests for the swipe feedback labels
#include "input/swipe_labels.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static void test_osu_shortcuts_are_named(void)
{
    char buf[16];
    swipe_label_t l = swipe_label_for(0x3B, 0, buf, sizeof(buf)); // F2
    assert(l.icon == SWIPE_ICON_SHUFFLE);
    assert(strcmp(l.label, "Random") == 0);
    // Same key with Shift: another shortcut
    l = swipe_label_for(0x3B, 0x02, buf, sizeof(buf));
    assert(strcmp(l.label, "Prev random") == 0);
    l = swipe_label_for(0x12, 0x01, buf, sizeof(buf)); // Ctrl+O
    assert(l.icon == SWIPE_ICON_SETTINGS);
    printf("✓ test_osu_shortcuts_are_named passed\n");
}

static void test_other_keys_show_their_name(void)
{
    char buf[16];
    swipe_label_t l = swipe_label_for(0x6A, 0, buf, sizeof(buf)); // F15
    assert(l.icon == SWIPE_ICON_KEYBOARD);
    assert(strcmp(l.label, "F15") == 0);
    l = swipe_label_for(0x3B, 0x01, buf, sizeof(buf)); // Ctrl+F2: no shortcut
    assert(strcmp(l.label, "F2") == 0);
    l = swipe_label_for(0x04, 0, buf, sizeof(buf));
    assert(strcmp(l.label, "Key 0x04") == 0);
    printf("✓ test_other_keys_show_their_name passed\n");
}

static void test_no_two_rows_share_a_key(void)
{
    for (unsigned i = 0; i < SWIPE_LABEL_COUNT; i++) {
        for (unsigned j = i + 1; j < SWIPE_LABEL_COUNT; j++) {
            assert(SWIPE_LABELS[i].key != SWIPE_LABELS[j].key ||
                   SWIPE_LABELS[i].mods != SWIPE_LABELS[j].mods);
        }
        // Fits the swipe box next to the icon
        assert(strlen(SWIPE_LABELS[i].label) <= 13);
    }
    printf("✓ test_no_two_rows_share_a_key passed\n");
}

int main(void)
{
    test_osu_shortcuts_are_named();
    test_other_keys_show_their_name();
    test_no_two_rows_share_a_key();
    printf("All swipe label tests passed\n");
    return 0;
}
