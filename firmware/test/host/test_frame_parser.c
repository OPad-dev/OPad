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
    frame_write_header(hdr, 0x1234);
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

    uint8_t chunk1[] = { 0xAA };
    uint8_t chunk2[] = { 0x55, 0x05, 0x00, 'H', 'E' };
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
        0xAA, 0x55, 0x03, 0x00, 'O', 'N', 'E',
        0xAA, 0x55, 0x03, 0x00, 'T', 'W', 'O',
    };

    frame_parser_feed(&parser, buffer, sizeof(buffer), test_handler, &ctx);
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
    frame_parser_feed(&parser, stream, sizeof(stream), test_handler, &ctx);
    assert(ctx.call_count == 1);
    assert(ctx.last_len == 2);
    assert(memcmp(ctx.last_payload, "OK", 2) == 0);
    assert(frame_parser_is_idle(&parser));
    assert(parser.resync_bytes == 12);

    printf("✓ test_stray_bytes_are_skipped passed\n");
}

static void test_text_command_leaves_parser_idle(void)
{
    frame_parser_t parser;
    frame_parser_init(&parser);
    test_context_t ctx = {0};

    const char *cmd = "FREAKY67\r\n";
    frame_parser_feed(&parser, (const uint8_t *)cmd, strlen(cmd), test_handler, &ctx);
    assert(ctx.call_count == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_text_command_leaves_parser_idle passed\n");
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

    frame_parser_feed(&parser, stream, sizeof(stream), test_handler, &ctx);
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
    frame_write_header(chunk1, 8000);

    frame_parser_feed(&parser, chunk1, sizeof(chunk1), test_handler, &ctx);
    assert(parser.rx_len == 4000);

    // 4000 + 5000 > PROTOCOL_MAX_FRAME_SIZE: the partial frame is dropped and
    // the new bytes are scanned; they end in a complete frame
    uint8_t chunk2[5000];
    memset(chunk2, 0xBB, sizeof(chunk2));
    uint8_t tail[] = {0xAA, 0x55, 0x02, 0x00, 'O', 'K'};
    memcpy(chunk2 + sizeof(chunk2) - sizeof(tail), tail, sizeof(tail));
    frame_parser_feed(&parser, chunk2, sizeof(chunk2), test_handler, &ctx);

    assert(parser.overflow_count == 1);
    assert(ctx.call_count == 1);
    assert(memcmp(ctx.last_payload, "OK", 2) == 0);
    assert(frame_parser_is_idle(&parser));

    printf("✓ test_buffer_overflow_recovery passed\n");
}

int main(void)
{
    test_header_encoding();
    test_single_frame();
    test_split_frame();
    test_back_to_back_frames();
    test_stray_bytes_are_skipped();
    test_text_command_leaves_parser_idle();
    test_oversized_frame_recovery();
    test_buffer_overflow_recovery();
    printf("All frame parser unit tests passed successfully!\n");
    return 0;
}
