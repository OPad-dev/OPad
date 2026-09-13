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

static void test_single_frame(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    uint8_t frame[] = {
        0x04, 0x00, 0x00, 0x00, // 4 bytes length
        'O', 'S', 'U', '!'       // payload
    };

    frame_parser_feed(&parser, frame, sizeof(frame), test_handler, &ctx);
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

    uint8_t chunk1[] = { 0x05, 0x00 };
    uint8_t chunk2[] = { 0x00, 0x00, 'H', 'E' };
    uint8_t chunk3[] = { 'L', 'L', 'O' };

    frame_parser_feed(&parser, chunk1, sizeof(chunk1), test_handler, &ctx);
    assert(ctx.call_count == 0);
    assert(!frame_parser_is_idle(&parser));

    frame_parser_feed(&parser, chunk2, sizeof(chunk2), test_handler, &ctx);
    assert(ctx.call_count == 0);

    frame_parser_feed(&parser, chunk3, sizeof(chunk3), test_handler, &ctx);
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
        // Frame 1 (3 bytes)
        0x03, 0x00, 0x00, 0x00,
        'O', 'N', 'E',
        // Frame 2 (3 bytes)
        0x03, 0x00, 0x00, 0x00,
        'T', 'W', 'O',
    };

    frame_parser_feed(&parser, buffer, sizeof(buffer), test_handler, &ctx);
    assert(ctx.call_count == 2);
    assert(ctx.last_len == 3);
    assert(memcmp(ctx.last_payload, "TWO", 3) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_back_to_back_frames passed\n");
}

static void test_oversized_frame_recovery(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    // Frame with length > 4096 - 4
    uint8_t oversized_header[] = {
        0xFF, 0xFF, 0x00, 0x00, // 65535 bytes
        0xAA, 0xBB, 0xCC, 0xDD,
    };

    frame_parser_feed(&parser, oversized_header, sizeof(oversized_header), test_handler, &ctx);
    assert(ctx.call_count == 0);
    assert(parser.oversized_count == 1);
    assert(frame_parser_is_idle(&parser)); // Drops buffer

    // Next valid frame must still parse properly
    uint8_t valid_frame[] = {
        0x02, 0x00, 0x00, 0x00,
        'O', 'K',
    };
    frame_parser_feed(&parser, valid_frame, sizeof(valid_frame), test_handler, &ctx);
    assert(ctx.call_count == 1);
    assert(ctx.last_len == 2);
    assert(memcmp(ctx.last_payload, "OK", 2) == 0);

    printf("✓ test_oversized_frame_recovery passed\n");
}

static void test_buffer_overflow_recovery(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    // Expected len = 8000 (valid within 8192)
    // 0x1F40 = 8000
    uint8_t chunk1[4000];
    memset(chunk1, 0xAA, sizeof(chunk1));
    chunk1[0] = 0x40;
    chunk1[1] = 0x1F;
    chunk1[2] = 0x00;
    chunk1[3] = 0x00;

    frame_parser_feed(&parser, chunk1, sizeof(chunk1), test_handler, &ctx);
    assert(parser.rx_len == 4000);

    // Now feed 5000 bytes: 4000 + 5000 = 9000 > PROTOCOL_MAX_FRAME_SIZE (8192)
    uint8_t chunk2[5000];
    memset(chunk2, 0xBB, sizeof(chunk2));
    frame_parser_feed(&parser, chunk2, sizeof(chunk2), test_handler, &ctx);

    assert(parser.overflow_count == 1);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_buffer_overflow_recovery passed\n");
}

int main(void)
{
    test_single_frame();
    test_split_frame();
    test_back_to_back_frames();
    test_oversized_frame_recovery();
    test_buffer_overflow_recovery();
    printf("All frame parser unit tests passed successfully!\n");
    return 0;
}
