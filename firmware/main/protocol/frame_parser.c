#include "frame_parser.h"
#include <string.h>

void frame_parser_init(frame_parser_t *parser)
{
    if (!parser) return;
    parser->rx_len = 0;
    parser->overflow_count = 0;
    parser->oversized_count = 0;
    parser->resync_bytes = 0;
    parser->last_format = FRAME_FORMAT_MARKED;
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

void frame_write_header(uint8_t out[FRAME_HEADER_SIZE], uint16_t payload_len,
                        frame_format_t format)
{
    if (format == FRAME_FORMAT_LEGACY) {
        out[0] = (uint8_t)(payload_len & 0xFF);
        out[1] = (uint8_t)(payload_len >> 8);
        out[2] = 0;
        out[3] = 0;
        return;
    }
    out[0] = FRAME_MAGIC_0;
    out[1] = FRAME_MAGIC_1;
    out[2] = (uint8_t)(payload_len & 0xFF);
    out[3] = (uint8_t)(payload_len >> 8);
}

void frame_parser_feed(frame_parser_t *parser, const uint8_t *data, size_t len,
                       frame_accept_t accept, frame_handler_t handler, void *user_data)
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

    // Consumed bytes are skipped with a head index and compacted away once at
    // the end, so sliding through junk costs one pass over it
    size_t head = 0;
    while (parser->rx_len - head >= FRAME_HEADER_SIZE) {
        // Both framings need the whole 4-byte header to be told apart
        const uint8_t *b = parser->rx_buf + head;

        frame_format_t format;
        size_t expected_len;
        if (accept != FRAME_ACCEPT_LEGACY && b[0] == FRAME_MAGIC_0 && b[1] == FRAME_MAGIC_1) {
            format = FRAME_FORMAT_MARKED;
            expected_len = (size_t)b[2] | ((size_t)b[3] << 8);
            if (expected_len > FRAME_MAX_PAYLOAD) {
                // Not a real header: slide past its first byte and keep looking
                parser->oversized_count++;
                parser->resync_bytes++;
                head++;
                continue;
            }
        } else if (accept == FRAME_ACCEPT_MARKED) {
            // Only a magic byte can start a frame: jump to the next one, but no
            // further than the last position with room for a whole header
            size_t stop = parser->rx_len - (FRAME_HEADER_SIZE - 1);
            const uint8_t *next = memchr(b + 1, FRAME_MAGIC_0, stop - (head + 1));
            size_t to = next ? (size_t)(next - parser->rx_buf) : stop;
            parser->resync_bytes += (uint32_t)(to - head);
            head = to;
            continue;
        } else {
            // Locked to legacy, AA 55 lands here and claims >= 0x55AA bytes: slid past
            format = FRAME_FORMAT_LEGACY;
            expected_len = (size_t)b[0] | ((size_t)b[1] << 8);
            if (b[2] != 0 || b[3] != 0 || expected_len < FRAME_LEGACY_MIN_PAYLOAD ||
                expected_len > FRAME_MAX_PAYLOAD) {
                parser->resync_bytes++;
                head++;
                continue;
            }
        }

        if (parser->rx_len - head < FRAME_HEADER_SIZE + expected_len) {
            break;
        }

        parser->last_format = format;
        if (handler) {
            handler(b + FRAME_HEADER_SIZE, expected_len, user_data);
        }
        head += FRAME_HEADER_SIZE + expected_len;
    }

    if (head > 0) {
        size_t remaining = parser->rx_len - head;
        if (remaining > 0) {
            memmove(parser->rx_buf, parser->rx_buf + head, remaining);
        }
        parser->rx_len = remaining;
    }
}
