#include "frame_parser.h"
#include <string.h>

void frame_parser_init(frame_parser_t *parser)
{
    if (!parser) return;
    parser->rx_len = 0;
    parser->overflow_count = 0;
    parser->oversized_count = 0;
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

void frame_parser_feed(frame_parser_t *parser, const uint8_t *data, size_t len,
                       frame_handler_t handler, void *user_data)
{
    if (!parser || !data || len == 0) return;

    if (parser->rx_len + len > sizeof(parser->rx_buf)) {
        parser->overflow_count++;
        parser->rx_len = 0;
        return;
    }

    memcpy(parser->rx_buf + parser->rx_len, data, len);
    parser->rx_len += len;

    while (parser->rx_len >= 4) {
        uint32_t expected_len = (uint32_t)parser->rx_buf[0] |
                                ((uint32_t)parser->rx_buf[1] << 8) |
                                ((uint32_t)parser->rx_buf[2] << 16) |
                                ((uint32_t)parser->rx_buf[3] << 24);

        if (expected_len > PROTOCOL_MAX_FRAME_SIZE - 4) {
            parser->oversized_count++;
            parser->rx_len = 0;
            return;
        }

        if (parser->rx_len < 4 + expected_len) {
            break;
        }

        if (handler) {
            handler(parser->rx_buf + 4, expected_len, user_data);
        }

        size_t consumed = 4 + expected_len;
        size_t remaining = parser->rx_len - consumed;
        if (remaining > 0) {
            memmove(parser->rx_buf, parser->rx_buf + consumed, remaining);
        }
        parser->rx_len = remaining;
    }
}
