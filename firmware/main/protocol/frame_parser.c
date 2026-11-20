#include "frame_parser.h"
#include <string.h>

void frame_parser_init(frame_parser_t *parser)
{
    if (!parser) return;
    parser->rx_len = 0;
    parser->overflow_count = 0;
    parser->oversized_count = 0;
    parser->resync_bytes = 0;
    parser->crc_errors = 0;
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

uint16_t frame_crc16(const uint8_t *data, size_t len)
{
    uint16_t crc = 0xFFFF;
    for (size_t i = 0; i < len; i++) {
        crc ^= (uint16_t)data[i] << 8;
        for (int b = 0; b < 8; b++) {
            crc = (crc & 0x8000) ? (uint16_t)((crc << 1) ^ 0x1021) : (uint16_t)(crc << 1);
        }
    }
    return crc;
}

// Covers the length bytes and the payload, so a damaged length fails too
static uint16_t checked_crc(const uint8_t *frame, size_t payload_len)
{
    return frame_crc16(frame + 2, 2 + payload_len);
}

void frame_write_crc(uint8_t *frame, size_t payload_len)
{
    uint16_t crc = checked_crc(frame, payload_len);
    frame[FRAME_HEADER_SIZE + payload_len] = (uint8_t)(crc & 0xFF);
    frame[FRAME_HEADER_SIZE + payload_len + 1] = (uint8_t)(crc >> 8);
}

size_t frame_trailer_size(frame_format_t format)
{
    return format == FRAME_FORMAT_CHECKED ? FRAME_CRC_SIZE : 0;
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
    out[1] = format == FRAME_FORMAT_CHECKED ? FRAME_MAGIC_1_CHECKED : FRAME_MAGIC_1;
    out[2] = (uint8_t)(payload_len & 0xFF);
    out[3] = (uint8_t)(payload_len >> 8);
}

// Delivers every whole frame from rx_buf[head] on, then compacts the rest
static void scan(frame_parser_t *parser, size_t head, frame_accept_t accept,
                 frame_handler_t handler, void *user_data)
{
    // Consumed bytes are skipped with a head index and compacted away once at
    // the end, so sliding through junk costs one pass over it
    while (parser->rx_len - head >= FRAME_HEADER_SIZE) {
        // Every framing needs the whole 4-byte header to be told apart
        const uint8_t *b = parser->rx_buf + head;

        frame_format_t format;
        size_t expected_len;
        bool marker = b[0] == FRAME_MAGIC_0 && (b[1] == FRAME_MAGIC_1 || b[1] == FRAME_MAGIC_1_CHECKED);
        if (accept != FRAME_ACCEPT_LEGACY && marker) {
            format = b[1] == FRAME_MAGIC_1_CHECKED ? FRAME_FORMAT_CHECKED : FRAME_FORMAT_MARKED;
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
            // Locked to legacy, AA 55 / AA 5A land here and claim >= 0x55AA bytes: slid past
            format = FRAME_FORMAT_LEGACY;
            expected_len = (size_t)b[0] | ((size_t)b[1] << 8);
            if (b[2] != 0 || b[3] != 0 || expected_len < FRAME_LEGACY_MIN_PAYLOAD ||
                expected_len > FRAME_MAX_PAYLOAD) {
                parser->resync_bytes++;
                head++;
                continue;
            }
        }

        size_t total = FRAME_HEADER_SIZE + expected_len + frame_trailer_size(format);
        if (parser->rx_len - head < total) {
            break;
        }
        if (format == FRAME_FORMAT_CHECKED) {
            uint16_t got = (uint16_t)b[FRAME_HEADER_SIZE + expected_len] |
                           (uint16_t)(b[FRAME_HEADER_SIZE + expected_len + 1] << 8);
            if (got != checked_crc(b, expected_len)) {
                // A cut-short frame with the next one glued on, or a false
                // start: the frames inside it are found from the next byte
                parser->crc_errors++;
                parser->resync_bytes++;
                head++;
                continue;
            }
        }

        parser->last_format = format;
        if (handler) {
            handler(b + FRAME_HEADER_SIZE, expected_len, user_data);
        }
        head += total;
    }

    if (head > 0) {
        size_t remaining = parser->rx_len - head;
        if (remaining > 0) {
            memmove(parser->rx_buf, parser->rx_buf + head, remaining);
        }
        parser->rx_len = remaining;
    }
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
    scan(parser, 0, accept, handler, user_data);
}

void frame_parser_expire(frame_parser_t *parser, frame_accept_t accept,
                         frame_handler_t handler, void *user_data)
{
    if (!parser || parser->rx_len == 0) return;
    bool checked = parser->rx_len >= 2 && parser->rx_buf[0] == FRAME_MAGIC_0 &&
                   parser->rx_buf[1] == FRAME_MAGIC_1_CHECKED;
    if (!checked) {
        parser->rx_len = 0;
        return;
    }
    parser->resync_bytes++;
    scan(parser, 1, accept, handler, user_data);
}
