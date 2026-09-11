#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "protocol/osupad.pb.h"

#ifdef __cplusplus
extern "C" {
#endif

#define PROTOCOL_MAX_FRAME_SIZE 4096

/**
 * @brief Encode a DeviceToHost protobuf message with a 4-byte LE length prefix.
 * @param msg Source message struct
 * @param out_buf Destination buffer (at least PROTOCOL_MAX_FRAME_SIZE bytes)
 * @param max_len Size of destination buffer
 * @param out_len Total bytes written (including 4-byte length prefix)
 */
esp_err_t protocol_encode_device_message(const osupad_DeviceToHost *msg, uint8_t *out_buf, size_t max_len, size_t *out_len);

/**
 * @brief Decode a HostToDevice protobuf message from raw payload bytes.
 * @param payload Pointer to raw protobuf bytes (excluding the 4-byte length prefix)
 * @param payload_len Length of payload bytes
 * @param out_msg Target decoded message struct
 */
bool protocol_decode_host_message(const uint8_t *payload, size_t payload_len, osupad_HostToDevice *out_msg);

/**
 * @brief Feed raw incoming serial CDC bytes into the protocol framing state machine.
 * When a complete frame is received, it decodes and executes the message handler.
 * @param data Received bytes from CDC
 * @param len Number of received bytes
 */
void protocol_feed_cdc_bytes(const uint8_t *data, size_t len);

/**
 * @brief Send current DeviceStatus to host over CDC.
 */
esp_err_t protocol_send_status(void);

/**
 * @brief Send HelloAck response to host over CDC.
 */
esp_err_t protocol_send_hello_ack(uint32_t seq);

/**
 * @brief Send ConfigAck response to host over CDC.
 */
esp_err_t protocol_send_config_ack(uint32_t seq, bool success, const char *msg);

/**
 * @brief Send CounterSyncResponse to host over CDC.
 */
esp_err_t protocol_send_counter_sync_resp(uint32_t seq, bool success);

#ifdef __cplusplus
}
#endif
