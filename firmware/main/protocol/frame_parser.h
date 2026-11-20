#pragma once

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifndef PROTOCOL_MAX_FRAME_SIZE
#define PROTOCOL_MAX_FRAME_SIZE 8192
#endif

/*
 * Three framings, all with a 4-byte header:
 *
 *   checked: [0xAA][0x5A][payload length, uint16 LE][payload][CRC-16 LE]
 *   marked:  [0xAA][0x55][payload length, uint16 LE][payload]
 *   legacy:  [payload length, uint32 LE][payload]   (hosts before the marker)
 *
 * Checked frames carry a CRC-16/CCITT-FALSE over the length and the payload.
 * They are used only once both sides said so (Hello.frame_crc and
 * HelloAck.frame_crc) and are read wherever marked frames are, so frames in
 * flight across the switch are never refused. A checked frame whose CRC does
 * not match is dropped, and the parser rescans from the byte after its
 * marker: a frame cut short by the host and glued to the next one costs only
 * the cut frame, not the next one as well.
 *
 * They cannot be confused: a legacy header starting AA 55 would claim at least
 * 0x55AA bytes, more than any frame may hold. The marker lets the parser find
 * the next frame after stray bytes by sliding one byte at a time; a legacy
 * header is only believed when its length is plausible (2..FRAME_MAX_PAYLOAD,
 * so its top two bytes are zero).
 *
 * Until the host's framing is known both are accepted. Once it is, the caller
 * locks the parser to it: plausible-looking headers of the other framing inside
 * a payload (any xx xx 00 00 reads as legacy) are then slid past, not believed.
 */
#define FRAME_MAGIC_0 0xAA
#define FRAME_MAGIC_1 0x55
#define FRAME_MAGIC_1_CHECKED 0x5A
#define FRAME_HEADER_SIZE 4
#define FRAME_CRC_SIZE 2
#define FRAME_MAX_PAYLOAD (PROTOCOL_MAX_FRAME_SIZE - FRAME_HEADER_SIZE - FRAME_CRC_SIZE)
/* Every real message carries at least a sequence number or a payload tag */
#define FRAME_LEGACY_MIN_PAYLOAD 2

typedef enum {
    FRAME_FORMAT_MARKED = 0,
    FRAME_FORMAT_LEGACY = 1,
    FRAME_FORMAT_CHECKED = 2,
} frame_format_t;

typedef enum {
    FRAME_ACCEPT_ANY = 0,
    FRAME_ACCEPT_MARKED,   // marked and checked frames
    FRAME_ACCEPT_LEGACY,
} frame_accept_t;

typedef struct {
    uint8_t rx_buf[PROTOCOL_MAX_FRAME_SIZE];
    size_t rx_len;
    uint32_t overflow_count;
    uint32_t oversized_count;
    uint32_t resync_bytes;   // bytes skipped looking for a frame start
    uint32_t crc_errors;     // checked frames dropped for a CRC mismatch
    frame_format_t last_format; // framing of the frame last handed to the handler
} frame_parser_t;

/** Writes the 4-byte header for a payload of payload_len bytes. */
void frame_write_header(uint8_t out[FRAME_HEADER_SIZE], uint16_t payload_len,
                        frame_format_t format);

/** CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF), the checked framing's */
uint16_t frame_crc16(const uint8_t *data, size_t len);

/**
 * For a checked frame whose header and payload are at frame[0 ..
 * FRAME_HEADER_SIZE + payload_len), writes the CRC after them. The caller
 * leaves FRAME_CRC_SIZE bytes of room.
 */
void frame_write_crc(uint8_t *frame, size_t payload_len);

/** Bytes after the payload: FRAME_CRC_SIZE for checked frames, else 0 */
size_t frame_trailer_size(frame_format_t format);

typedef void (*frame_handler_t)(const uint8_t *payload, size_t payload_len, void *user_data);

void frame_parser_init(frame_parser_t *parser);
void frame_parser_reset(frame_parser_t *parser);

/**
 * A partial frame waited too long. A checked one is dropped by its first byte
 * and the rest rescanned (whole frames behind it are delivered, its CRC
 * protects against false starts); anything else is dropped whole.
 */
void frame_parser_expire(frame_parser_t *parser, frame_accept_t accept,
                         frame_handler_t handler, void *user_data);
bool frame_parser_is_idle(const frame_parser_t *parser);

void frame_parser_feed(frame_parser_t *parser, const uint8_t *data, size_t len,
                       frame_accept_t accept, frame_handler_t handler, void *user_data);
