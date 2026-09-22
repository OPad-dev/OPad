#pragma once

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifndef PROTOCOL_MAX_FRAME_SIZE
#define PROTOCOL_MAX_FRAME_SIZE 8192
#endif

/*
 * Two framings, both with a 4-byte header, both accepted:
 *
 *   marked: [0xAA][0x55][payload length, uint16 LE][payload]
 *   legacy: [payload length, uint32 LE][payload]   (hosts before the marker)
 *
 * They cannot be confused: a legacy header starting AA 55 would claim at least
 * 0x55AA bytes, more than any frame may hold. The marker lets the parser find
 * the next frame after stray bytes by sliding one byte at a time; a legacy
 * header is only believed when its length is plausible (2..FRAME_MAX_PAYLOAD,
 * so its top two bytes are zero).
 */
#define FRAME_MAGIC_0 0xAA
#define FRAME_MAGIC_1 0x55
#define FRAME_HEADER_SIZE 4
#define FRAME_MAX_PAYLOAD (PROTOCOL_MAX_FRAME_SIZE - FRAME_HEADER_SIZE)
/* Every real message carries at least a sequence number or a payload tag */
#define FRAME_LEGACY_MIN_PAYLOAD 2

typedef enum {
    FRAME_FORMAT_MARKED = 0,
    FRAME_FORMAT_LEGACY = 1,
} frame_format_t;

typedef struct {
    uint8_t rx_buf[PROTOCOL_MAX_FRAME_SIZE];
    size_t rx_len;
    uint32_t overflow_count;
    uint32_t oversized_count;
    uint32_t resync_bytes;   // bytes skipped looking for a frame start
    frame_format_t last_format; // framing of the frame last handed to the handler
} frame_parser_t;

/** Writes the 4-byte header for a payload of payload_len bytes. */
void frame_write_header(uint8_t out[FRAME_HEADER_SIZE], uint16_t payload_len,
                        frame_format_t format);

typedef void (*frame_handler_t)(const uint8_t *payload, size_t payload_len, void *user_data);

void frame_parser_init(frame_parser_t *parser);
void frame_parser_reset(frame_parser_t *parser);
bool frame_parser_is_idle(const frame_parser_t *parser);

void frame_parser_feed(frame_parser_t *parser, const uint8_t *data, size_t len,
                       frame_handler_t handler, void *user_data);
