/*
 * Game keys (input/game_keys.h): which keys the screen's two key cards show,
 * what the totals add up, and the card labels.
 */
#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "../../main/input/game_keys.h"

static void test_slots(void)
{
    int s[2];
    game_keys_slots(0x3, s);   // Z/X, the default: cards as they always were
    assert(s[0] == 0 && s[1] == 1);
    game_keys_slots(0x5, s);   // Z/C: key 3 on the second card
    assert(s[0] == 0 && s[1] == 2);
    game_keys_slots(0x6, s);   // X/C
    assert(s[0] == 1 && s[1] == 2);
    game_keys_slots(0x7, s);   // all three: the lowest two get cards
    assert(s[0] == 0 && s[1] == 1);
    game_keys_slots(0x4, s);   // one key only
    assert(s[0] == 2 && s[1] == -1);
    printf("✓ test_slots passed\n");
}

static void test_sum(void)
{
    const uint64_t v[3] = {10, 200, 3000};
    assert(game_keys_sum(0x3, v) == 210);
    assert(game_keys_sum(0x5, v) == 3010);
    assert(game_keys_sum(0x7, v) == 3210);
    printf("✓ test_sum passed\n");
}

static void test_labels(void)
{
    char l[2];
    game_keys_label(0x1D, l);
    assert(strcmp(l, "Z") == 0);
    game_keys_label(0x06, l);
    assert(strcmp(l, "C") == 0);
    game_keys_label(0x1E, l);
    assert(strcmp(l, "1") == 0);
    game_keys_label(0x27, l);
    assert(strcmp(l, "0") == 0);
    game_keys_label(0x2C, l);  // Space: the layout's own text stays
    assert(l[0] == '\0');
    game_keys_label(0, l);
    assert(l[0] == '\0');
    printf("✓ test_labels passed\n");
}

int main(void)
{
    test_slots();
    test_sum();
    test_labels();
    return 0;
}
