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
    printf("All frame parser unit tests passed successfully!\n");
    return 0;
}
