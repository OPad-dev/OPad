#pragma once

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifndef PROTOCOL_MAX_FRAME_SIZE
#define PROTOCOL_MAX_FRAME_SIZE 8192
#endif

typedef struct {
    uint8_t rx_buf[PROTOCOL_MAX_FRAME_SIZE];
    size_t rx_len;
    uint32_t overflow_count;
    uint32_t oversized_count;
} frame_parser_t;

typedef void (*frame_handler_t)(const uint8_t *payload, size_t payload_len, void *user_data);

void frame_parser_init(frame_parser_t *parser);
void frame_parser_reset(frame_parser_t *parser);
bool frame_parser_is_idle(const frame_parser_t *parser);

void frame_parser_feed(frame_parser_t *parser, const uint8_t *data, size_t len,
                       frame_handler_t handler, void *user_data);
