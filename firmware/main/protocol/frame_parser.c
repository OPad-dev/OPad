#include "frame_parser.h"
#include <string.h>

void frame_parser_init(frame_parser_t *parser)
{
    if (!parser) return;
    parser->rx_len = 0;
    parser->overflow_count = 0;
    parser->oversized_count = 0;
    parser->resync_bytes = 0;
}

void frame_parser_reset(frame_parser_t *parser)
{
    if (!parser) return;
    parser->rx_len = 0;
}

bool frame_parser_is_idle(const frame_parser_t *parser)
{
    return parser ? (parser->rx_len == 0) : true;
}

void frame_write_header(uint8_t out[FRAME_HEADER_SIZE], uint16_t payload_len)
{
    out[0] = FRAME_MAGIC_0;
    out[1] = FRAME_MAGIC_1;
    out[2] = (uint8_t)(payload_len & 0xFF);
    out[3] = (uint8_t)(payload_len >> 8);
}

static void drop_front(frame_parser_t *parser, size_t n)
{
    size_t remaining = parser->rx_len - n;
    if (remaining > 0) {
        memmove(parser->rx_buf, parser->rx_buf + n, remaining);
    }
    parser->rx_len = remaining;
}

void frame_parser_feed(frame_parser_t *parser, const uint8_t *data, size_t len,
                       frame_handler_t handler, void *user_data)
{
    if (!parser || !data || len == 0) return;

    if (len > sizeof(parser->rx_buf)) {
        parser->overflow_count++;
        parser->rx_len = 0;
        return;
    }
    if (parser->rx_len + len > sizeof(parser->rx_buf)) {
        // A frame that cannot fit is garbage: drop it and scan the new bytes
        parser->overflow_count++;
        parser->rx_len = 0;
    }

    memcpy(parser->rx_buf + parser->rx_len, data, len);
    parser->rx_len += len;

    while (parser->rx_len > 0) {
        if (parser->rx_buf[0] != FRAME_MAGIC_0) {
            // Skip straight to the next candidate start byte
            const uint8_t *next = memchr(parser->rx_buf + 1, FRAME_MAGIC_0, parser->rx_len - 1);
            size_t skip = next ? (size_t)(next - parser->rx_buf) : parser->rx_len;
            parser->resync_bytes += skip;
            drop_front(parser, skip);
            continue;
        }
        if (parser->rx_len < 2) {
            break;
        }
        if (parser->rx_buf[1] != FRAME_MAGIC_1) {
            parser->resync_bytes++;
            drop_front(parser, 1);
            continue;
        }
        if (parser->rx_len < FRAME_HEADER_SIZE) {
            break;
        }

        size_t expected_len = (size_t)parser->rx_buf[2] | ((size_t)parser->rx_buf[3] << 8);
        if (expected_len > FRAME_MAX_PAYLOAD) {
            // Not a real header: slide past its first byte and keep looking
            parser->oversized_count++;
            parser->resync_bytes++;
            drop_front(parser, 1);
            continue;
        }

        if (parser->rx_len < FRAME_HEADER_SIZE + expected_len) {
            break;
        }

        if (handler) {
            handler(parser->rx_buf + FRAME_HEADER_SIZE, expected_len, user_data);
        }
        drop_front(parser, FRAME_HEADER_SIZE + expected_len);
    }
}
