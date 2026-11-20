#include <stdio.h>
#include <assert.h>
#include <string.h>
#include "protocol/frame_parser.h"

typedef struct {
    uint32_t call_count;
    uint8_t last_payload[256];
    size_t last_len;
} test_context_t;

static void test_handler(const uint8_t *payload, size_t len, void *user_data)
{
    test_context_t *ctx = (test_context_t *)user_data;
    ctx->call_count++;
    ctx->last_len = len;
    if (len <= sizeof(ctx->last_payload)) {
        memcpy(ctx->last_payload, payload, len);
    }
}

static void test_header_encoding(void)
{
    uint8_t hdr[FRAME_HEADER_SIZE];
    frame_write_header(hdr, 0x1234, FRAME_FORMAT_MARKED);
    assert(hdr[0] == 0xAA && hdr[1] == 0x55 && hdr[2] == 0x34 && hdr[3] == 0x12);
    printf("✓ test_header_encoding passed\n");
}

static void test_single_frame(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    uint8_t frame[] = {
        0xAA, 0x55, 0x04, 0x00, // magic + 4 bytes length
        'O', 'S', 'U', '!'       // payload
    };

    frame_parser_feed(&parser, frame, sizeof(frame), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 1);
    assert(ctx.last_len == 4);
    assert(memcmp(ctx.last_payload, "OSU!", 4) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_single_frame passed\n");
}

static void test_split_frame(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    uint8_t chunk1[] = { 0xAA };
    uint8_t chunk2[] = { 0x55, 0x05, 0x00, 'H', 'E' };
    uint8_t chunk3[] = { 'L', 'L', 'O' };

    frame_parser_feed(&parser, chunk1, sizeof(chunk1), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 0);
    assert(!frame_parser_is_idle(&parser));

    frame_parser_feed(&parser, chunk2, sizeof(chunk2), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 0);

    frame_parser_feed(&parser, chunk3, sizeof(chunk3), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 1);
    assert(ctx.last_len == 5);
    assert(memcmp(ctx.last_payload, "HELLO", 5) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_split_frame passed\n");
}

static void test_back_to_back_frames(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    uint8_t buffer[] = {
        0xAA, 0x55, 0x03, 0x00, 'O', 'N', 'E',
        0xAA, 0x55, 0x03, 0x00, 'T', 'W', 'O',
    };

    frame_parser_feed(&parser, buffer, sizeof(buffer), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 2);
    assert(ctx.last_len == 3);
    assert(memcmp(ctx.last_payload, "TWO", 3) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_back_to_back_frames passed\n");
}

static void test_stray_bytes_are_skipped(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    // Bootloader chatter, a lone 0xAA, and 0xAA 0xAA before a real frame
    uint8_t stream[] = {
        'E', 'S', 'P', '-', 'R', 'O', 'M', '\r', '\n',
        0xAA, 0x12,
        0xAA, 0xAA, 0x55, 0x02, 0x00, 'O', 'K',
    };
    frame_parser_feed(&parser, stream, sizeof(stream), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 1);
    assert(ctx.last_len == 2);
    assert(memcmp(ctx.last_payload, "OK", 2) == 0);
    assert(frame_parser_is_idle(&parser));
    assert(parser.resync_bytes == 12);

    printf("✓ test_stray_bytes_are_skipped passed\n");
}

static void test_stray_text_then_a_frame(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    const char *cmd = "FREAKY67\r\n";
    frame_parser_feed(&parser, (const uint8_t *)cmd, strlen(cmd), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 0);

    uint8_t frame[] = {0xAA, 0x55, 0x02, 0x00, 'O', 'K'};
    frame_parser_feed(&parser, frame, sizeof(frame), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 1);
    assert(memcmp(ctx.last_payload, "OK", 2) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_stray_text_then_a_frame passed\n");
}

static void test_legacy_frames_from_old_hosts(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    // [u32 LE length][payload], as hosts before the AA 55 marker send it
    uint8_t frame[] = {0x05, 0x00, 0x00, 0x00, 'H', 'E', 'L', 'L', 'O'};
    frame_parser_feed(&parser, frame, sizeof(frame), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 1);
    assert(ctx.last_len == 5);
    assert(memcmp(ctx.last_payload, "HELLO", 5) == 0);
    assert(parser.last_format == FRAME_FORMAT_LEGACY);
    assert(frame_parser_is_idle(&parser));

    // A legacy length whose low byte is 0xAA (170) is still legacy
    uint8_t big[4 + 170];
    memset(big, 'x', sizeof(big));
    frame_write_header(big, 170, FRAME_FORMAT_LEGACY);
    assert(big[0] == 0xAA && big[1] == 0x00 && big[2] == 0 && big[3] == 0);
    frame_parser_feed(&parser, big, sizeof(big), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 2);
    assert(ctx.last_len == 170);
    assert(parser.last_format == FRAME_FORMAT_LEGACY);

    // And a marked frame right after switches the reported format back
    uint8_t marked[] = {0xAA, 0x55, 0x02, 0x00, 'O', 'K'};
    frame_parser_feed(&parser, marked, sizeof(marked), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 3);
    assert(parser.last_format == FRAME_FORMAT_MARKED);

    printf("✓ test_legacy_frames_from_old_hosts passed\n");
}

static void test_legacy_header_encoding(void)
{
    uint8_t hdr[FRAME_HEADER_SIZE];
    frame_write_header(hdr, 0x1234, FRAME_FORMAT_LEGACY);
    assert(hdr[0] == 0x34 && hdr[1] == 0x12 && hdr[2] == 0 && hdr[3] == 0);
    printf("✓ test_legacy_header_encoding passed\n");
}

static void test_oversized_frame_recovery(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    // Magic followed by a length the device can never accept, then a real frame
    uint8_t stream[] = {
        0xAA, 0x55, 0xFF, 0xFF,
        0xAA, 0x55, 0x02, 0x00, 'O', 'K',
    };

    frame_parser_feed(&parser, stream, sizeof(stream), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(parser.oversized_count == 1);
    assert(ctx.call_count == 1);
    assert(ctx.last_len == 2);
    assert(memcmp(ctx.last_payload, "OK", 2) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_oversized_frame_recovery passed\n");
}

static void test_buffer_overflow_recovery(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    // Expected len = 8000 (valid within 8192), only half of it arrives
    uint8_t chunk1[4000];
    memset(chunk1, 0x11, sizeof(chunk1));
    frame_write_header(chunk1, 8000, FRAME_FORMAT_MARKED);

    frame_parser_feed(&parser, chunk1, sizeof(chunk1), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(parser.rx_len == 4000);

    // 4000 + 5000 > PROTOCOL_MAX_FRAME_SIZE: the partial frame is dropped and
    // the new bytes are scanned; they end in a complete frame
    uint8_t chunk2[5000];
    memset(chunk2, 0xBB, sizeof(chunk2));
    uint8_t tail[] = {0xAA, 0x55, 0x02, 0x00, 'O', 'K'};
    memcpy(chunk2 + sizeof(chunk2) - sizeof(tail), tail, sizeof(tail));
    frame_parser_feed(&parser, chunk2, sizeof(chunk2), FRAME_ACCEPT_ANY, test_handler, &ctx);

    assert(parser.overflow_count == 1);
    assert(ctx.call_count == 1);
    assert(memcmp(ctx.last_payload, "OK", 2) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_buffer_overflow_recovery passed\n");
}

// A marked host after lost bytes: 09 00 00 00 reads as a legacy header that
// would swallow the next real frame
static void test_locked_marked_ignores_legacy_headers(void)
{
    const uint8_t stream[] = {
        0x09, 0x00, 0x00, 0x00,
        0xAA, 0x55, 0x02, 0x00, 'O', 'K',
        0xAA, 0x55, 0x02, 0x00, 'H', 'I',
    };

    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    frame_parser_feed(&parser, stream, sizeof(stream), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 1 && ctx.last_len == 9); // captured, both frames lost

    frame_parser_init(&parser);
    memset(&ctx, 0, sizeof(ctx));
    frame_parser_feed(&parser, stream, sizeof(stream), FRAME_ACCEPT_MARKED, test_handler, &ctx);
    assert(ctx.call_count == 2);
    assert(ctx.last_len == 2 && memcmp(ctx.last_payload, "HI", 2) == 0);
    assert(parser.resync_bytes == 4);
    assert(parser.last_format == FRAME_FORMAT_MARKED);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_locked_marked_ignores_legacy_headers passed\n");
}

// A legacy host: a stray AA 55 00 00 is a zero-length marked frame to an
// unlocked parser, and nothing to one locked to legacy
static void test_locked_legacy_ignores_marked_headers(void)
{
    uint8_t stream[4 + 4 + 32];
    const uint8_t stray[] = {0xAA, 0x55, 0x00, 0x00};
    memcpy(stream, stray, sizeof(stray));
    frame_write_header(stream + 4, 32, FRAME_FORMAT_LEGACY);
    memset(stream + 8, 'x', 32);

    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    frame_parser_feed(&parser, stream, sizeof(stream), FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 2);

    frame_parser_init(&parser);
    memset(&ctx, 0, sizeof(ctx));
    frame_parser_feed(&parser, stream, sizeof(stream), FRAME_ACCEPT_LEGACY, test_handler, &ctx);
    assert(ctx.call_count == 1 && ctx.last_len == 32);
    assert(parser.last_format == FRAME_FORMAT_LEGACY);
    assert(parser.resync_bytes == 4);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_locked_legacy_ignores_marked_headers passed\n");
}

static void test_unlocked_accepts_both_framings(void)
{
    const uint8_t stream[] = {
        0xAA, 0x55, 0x02, 0x00, 'O', 'K',
        0x02, 0x00, 0x00, 0x00, 'H', 'I',
    };
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    frame_parser_feed(&parser, stream, 6, FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 1 && parser.last_format == FRAME_FORMAT_MARKED);
    frame_parser_feed(&parser, stream + 6, 6, FRAME_ACCEPT_ANY, test_handler, &ctx);
    assert(ctx.call_count == 2 && parser.last_format == FRAME_FORMAT_LEGACY);
    assert(memcmp(ctx.last_payload, "HI", 2) == 0);

    printf("✓ test_unlocked_accepts_both_framings passed\n");
}

// 8 KB of bootloader chatter with false marked headers in it, fed as one
// buffer and as 256-byte CDC reads: the marked frame after it is found
static void fill_junk(uint8_t *buf, size_t len)
{
    static const uint8_t pattern[] = {
        'E', 'S', 'P', '-', 'R', 'O', 'M', '\r', '\n',
        0xAA, 0x12,
        0xAA, 0x55, 0xFF, 0xFF,
    };
    for (size_t i = 0; i < len; i++) {
        buf[i] = pattern[i % sizeof(pattern)];
    }
}

static void test_resync_through_8k_of_junk(void)
{
    static const frame_accept_t modes[] = {FRAME_ACCEPT_ANY, FRAME_ACCEPT_MARKED};
    static uint8_t junk[8192];
    static uint8_t stream[PROTOCOL_MAX_FRAME_SIZE];
    const uint8_t frame[] = {0xAA, 0x55, 0x02, 0x00, 'O', 'K'};
    fill_junk(junk, sizeof(junk));

    for (size_t m = 0; m < sizeof(modes) / sizeof(modes[0]); m++) {
        // One buffer: as much junk as fits alongside the frame
        frame_parser_t parser;
        frame_parser_init(&parser);
        test_context_t ctx = {0};
        size_t junk_len = sizeof(stream) - sizeof(frame);
        memcpy(stream, junk, junk_len);
        memcpy(stream + junk_len, frame, sizeof(frame));
        frame_parser_feed(&parser, stream, sizeof(stream), modes[m], test_handler, &ctx);
        assert(ctx.call_count == 1 && ctx.last_len == 2);
        assert(memcmp(ctx.last_payload, "OK", 2) == 0);
        assert(parser.resync_bytes == junk_len);
        assert(parser.overflow_count == 0);
        assert(frame_parser_is_idle(&parser));

        // CDC-sized reads: the buffer never fills, junk never forms a frame
        frame_parser_init(&parser);
        memset(&ctx, 0, sizeof(ctx));
        for (size_t off = 0; off < sizeof(junk); off += 256) {
            frame_parser_feed(&parser, junk + off, 256, modes[m], test_handler, &ctx);
            assert(parser.rx_len < FRAME_HEADER_SIZE);
        }
        assert(ctx.call_count == 0);
        frame_parser_feed(&parser, frame, sizeof(frame), modes[m], test_handler, &ctx);
        assert(ctx.call_count == 1 && ctx.last_len == 2);
        assert(memcmp(ctx.last_payload, "OK", 2) == 0);
        assert(parser.resync_bytes == sizeof(junk));
        assert(parser.overflow_count == 0);
        assert(frame_parser_is_idle(&parser));
    }

    // Locked to legacy the junk is slid through the same way. A byte right
    // before a legacy header always makes a plausible length with it, so the
    // frame itself is found after the stale-partial reset, as on the pad.
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    for (size_t off = 0; off < sizeof(junk); off += 256) {
        frame_parser_feed(&parser, junk + off, 256, FRAME_ACCEPT_LEGACY, test_handler, &ctx);
        assert(parser.rx_len < FRAME_HEADER_SIZE);
    }
    assert(ctx.call_count == 0);
    assert(parser.resync_bytes == sizeof(junk) - parser.rx_len);
    frame_parser_reset(&parser);
    const uint8_t legacy[] = {0x02, 0x00, 0x00, 0x00, 'H', 'I'};
    frame_parser_feed(&parser, legacy, sizeof(legacy), FRAME_ACCEPT_LEGACY, test_handler, &ctx);
    assert(ctx.call_count == 1 && memcmp(ctx.last_payload, "HI", 2) == 0);

    printf("✓ test_resync_through_8k_of_junk passed\n");
}

// A checked frame for payload, written by the same code the pad sends with
static size_t make_checked(uint8_t *out, const char *payload)
{
    size_t len = strlen(payload);
    frame_write_header(out, (uint16_t)len, FRAME_FORMAT_CHECKED);
    memcpy(out + FRAME_HEADER_SIZE, payload, len);
    frame_write_crc(out, len);
    return FRAME_HEADER_SIZE + len + FRAME_CRC_SIZE;
}

static void test_crc16_check_value(void)
{
    // CRC-16/CCITT-FALSE of "123456789" is 0x29B1
    assert(frame_crc16((const uint8_t *)"123456789", 9) == 0x29B1);
    uint8_t f[32];
    size_t n = make_checked(f, "HI");
    assert(n == 4 + 2 + 2 && f[0] == 0xAA && f[1] == 0x5A && f[2] == 2 && f[3] == 0);
    printf("✓ test_crc16_check_value passed\n");
}

static void test_checked_frames_are_read_where_marked_ones_are(void)
{
    uint8_t f[64];
    size_t n = make_checked(f, "OSU!");
    frame_accept_t modes[] = {FRAME_ACCEPT_ANY, FRAME_ACCEPT_MARKED};
    for (int i = 0; i < 2; i++) {
        frame_parser_t parser;
        frame_parser_init(&parser);
        test_context_t ctx = {0};
        frame_parser_feed(&parser, f, n, modes[i], test_handler, &ctx);
        assert(ctx.call_count == 1 && ctx.last_len == 4 && memcmp(ctx.last_payload, "OSU!", 4) == 0);
        assert(parser.last_format == FRAME_FORMAT_CHECKED && frame_parser_is_idle(&parser));
    }
    // A host locked to legacy never sends one, and one is not believed
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    frame_parser_feed(&parser, f, n, FRAME_ACCEPT_LEGACY, test_handler, &ctx);
    assert(ctx.call_count == 0);
    printf("✓ test_checked_frames_are_read_where_marked_ones_are passed\n");
}

static void test_a_bad_crc_drops_only_that_frame(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    uint8_t buf[64];
    size_t a = make_checked(buf, "BAD!");
    buf[5] ^= 0x01;  // one flipped bit in the payload
    size_t b = make_checked(buf + a, "GOOD");
    frame_parser_feed(&parser, buf, a + b, FRAME_ACCEPT_MARKED, test_handler, &ctx);
    assert(ctx.call_count == 1 && memcmp(ctx.last_payload, "GOOD", 4) == 0);
    assert(parser.crc_errors == 1 && frame_parser_is_idle(&parser));
    printf("✓ test_a_bad_crc_drops_only_that_frame passed\n");
}

// The 2026-11 drop: the host gave up on a frame half-way and sent the next
// one. Unchecked, the pad glued them together and lost both.
static void test_a_cut_short_frame_does_not_take_the_next_one(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    uint8_t full[128], next[64];
    char long_payload[61];
    memset(long_payload, 'x', 60);
    long_payload[60] = 0;
    make_checked(full, long_payload);
    size_t sent = 4 + 20;  // header and a third of the payload, then the write gave up
    size_t n = make_checked(next, "NEXT MESSAGE, WHOLE, AND LONG ENOUGH TO COVER THE REST");
    frame_parser_feed(&parser, full, sent, FRAME_ACCEPT_MARKED, test_handler, &ctx);
    frame_parser_feed(&parser, next, n, FRAME_ACCEPT_MARKED, test_handler, &ctx);
    assert(ctx.call_count == 1 && memcmp(ctx.last_payload, "NEXT MESSAGE", 12) == 0);
    assert(parser.crc_errors == 1);
    printf("✓ test_a_cut_short_frame_does_not_take_the_next_one passed\n");
}

// The cut frame claims more than arrives before the line goes quiet: on
// expiry a checked partial gives up one byte and the whole frames behind it
// are still delivered; an unchecked one is dropped as before.
static void test_an_expired_checked_partial_keeps_the_frames_behind_it(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};
    uint8_t full[300], next[32];
    char big[201];
    memset(big, 'y', 200);
    big[200] = 0;
    make_checked(full, big);
    size_t n = make_checked(next, "SHORT");
    frame_parser_feed(&parser, full, 4 + 10, FRAME_ACCEPT_MARKED, test_handler, &ctx);
    frame_parser_feed(&parser, next, n, FRAME_ACCEPT_MARKED, test_handler, &ctx);
    assert(ctx.call_count == 0);  // still waiting for 200 bytes
    frame_parser_expire(&parser, FRAME_ACCEPT_MARKED, test_handler, &ctx);
    assert(ctx.call_count == 1 && memcmp(ctx.last_payload, "SHORT", 5) == 0);
    assert(frame_parser_is_idle(&parser));

    // Unchecked partials expire whole
    uint8_t marked[] = {0xAA, 0x55, 0x40, 0x00, 'a', 'b'};
    frame_parser_feed(&parser, marked, sizeof(marked), FRAME_ACCEPT_MARKED, test_handler, &ctx);
    frame_parser_expire(&parser, FRAME_ACCEPT_MARKED, test_handler, &ctx);
    assert(frame_parser_is_idle(&parser) && ctx.call_count == 1);
    printf("✓ test_an_expired_checked_partial_keeps_the_frames_behind_it passed\n");
}

int main(void)
{
    test_header_encoding();
    test_single_frame();
    test_split_frame();
    test_back_to_back_frames();
    test_stray_bytes_are_skipped();
    test_stray_text_then_a_frame();
    test_legacy_frames_from_old_hosts();
    test_legacy_header_encoding();
    test_oversized_frame_recovery();
    test_buffer_overflow_recovery();
    test_locked_marked_ignores_legacy_headers();
    test_locked_legacy_ignores_marked_headers();
    test_unlocked_accepts_both_framings();
    test_resync_through_8k_of_junk();
    test_crc16_check_value();
    test_checked_frames_are_read_where_marked_ones_are();
    test_a_bad_crc_drops_only_that_frame();
    test_a_cut_short_frame_does_not_take_the_next_one();
    test_an_expired_checked_partial_keeps_the_frames_behind_it();
    printf("All frame parser unit tests passed successfully!\n");
    return 0;
}
