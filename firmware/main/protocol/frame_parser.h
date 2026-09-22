#pragma once

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifndef PROTOCOL_MAX_FRAME_SIZE
#define PROTOCOL_MAX_FRAME_SIZE 8192
#endif

/*
 * Frame: [0xAA][0x55][payload length, uint16 LE][payload]. The magic lets the
 * parser find the next frame after stray bytes (bootloader chatter, a text
 * command, a dropped byte) by sliding one byte at a time.
 */
#define FRAME_MAGIC_0 0xAA
#define FRAME_MAGIC_1 0x55
#define FRAME_HEADER_SIZE 4
#define FRAME_MAX_PAYLOAD (PROTOCOL_MAX_FRAME_SIZE - FRAME_HEADER_SIZE)

typedef struct {
    uint8_t rx_buf[PROTOCOL_MAX_FRAME_SIZE];
    size_t rx_len;
    uint32_t overflow_count;
    uint32_t oversized_count;
    uint32_t resync_bytes;   // bytes skipped looking for a frame start
} frame_parser_t;

/** Writes the 4-byte header for a payload of payload_len bytes. */
void frame_write_header(uint8_t out[FRAME_HEADER_SIZE], uint16_t payload_len);

typedef void (*frame_handler_t)(const uint8_t *payload, size_t payload_len, void *user_data);

void frame_parser_init(frame_parser_t *parser);
void frame_parser_reset(frame_parser_t *parser);
bool frame_parser_is_idle(const frame_parser_t *parser);

void frame_parser_feed(frame_parser_t *parser, const uint8_t *data, size_t len,
                       frame_handler_t handler, void *user_data);
