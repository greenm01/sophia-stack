#include "sophia_wm_v1.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define FRAME_CAPACITY (24u + 65536u)

static int hex_nibble(char character) {
    if (character >= '0' && character <= '9') return character - '0';
    if (character >= 'a' && character <= 'f') return character - 'a' + 10;
    return -1;
}

static size_t decode_hex(const char *text, uint8_t *out, size_t capacity) {
    size_t length = strlen(text);
    if ((length % 2u) != 0 || length / 2u > capacity) return 0;
    for (size_t index = 0; index < length / 2u; ++index) {
        int high = hex_nibble(text[index * 2u]);
        int low = hex_nibble(text[index * 2u + 1u]);
        if (high < 0 || low < 0) return 0;
        out[index] = (uint8_t)((high << 4) | low);
    }
    return length / 2u;
}

static char *next_field(char **cursor) {
    char *field = *cursor;
    char *separator = strchr(field, '|');
    if (separator != NULL) {
        *separator = '\0';
        *cursor = separator + 1;
    } else {
        *cursor = NULL;
    }
    return field;
}

#define ROUNDTRIP_ZERO(name) do { \
    struct sophia_wm_v1_##name message; \
    status = sophia_wm_v1_decode_##name(frame, frame_len, &message); \
    if (status == SOPHIA_WM_V1_OK) \
        status = sophia_wm_v1_encode_##name(&message, encoded, sizeof(encoded), &encoded_len); \
} while (0)

#define ROUNDTRIP_REQUIRED(name) do { \
    struct sophia_wm_v1_##name message; \
    uint64_t decoded_transaction = 0; \
    status = sophia_wm_v1_decode_##name(frame, frame_len, &decoded_transaction, &message); \
    if (status == SOPHIA_WM_V1_OK && decoded_transaction != transaction) return 0; \
    if (status == SOPHIA_WM_V1_OK) \
        status = sophia_wm_v1_encode_##name(decoded_transaction, &message, encoded, sizeof(encoded), &encoded_len); \
} while (0)

static int roundtrip(const char *name, uint64_t transaction, const uint8_t *frame, size_t frame_len) {
    uint8_t encoded[FRAME_CAPACITY];
    size_t encoded_len = 0;
    enum sophia_wm_v1_status status = SOPHIA_WM_V1_WRONG_MESSAGE_KIND;

    if (strcmp(name, "client_hello") == 0) ROUNDTRIP_ZERO(client_hello);
    else if (strcmp(name, "server_welcome") == 0) ROUNDTRIP_ZERO(server_welcome);
    else if (strcmp(name, "snapshot_begin") == 0) ROUNDTRIP_REQUIRED(snapshot_begin);
    else if (strcmp(name, "snapshot_chunk") == 0) ROUNDTRIP_REQUIRED(snapshot_chunk);
    else if (strcmp(name, "snapshot_end") == 0) ROUNDTRIP_REQUIRED(snapshot_end);
    else if (strcmp(name, "projection_request") == 0) ROUNDTRIP_REQUIRED(projection_request);
    else if (strcmp(name, "projection_begin") == 0) ROUNDTRIP_REQUIRED(projection_begin);
    else if (strcmp(name, "projection_chunk") == 0) ROUNDTRIP_REQUIRED(projection_chunk);
    else if (strcmp(name, "projection_end") == 0) ROUNDTRIP_REQUIRED(projection_end);
    else if (strcmp(name, "projection_outcome") == 0) ROUNDTRIP_REQUIRED(projection_outcome);
    else if (strcmp(name, "policy_configuration") == 0) ROUNDTRIP_REQUIRED(policy_configuration);
    else if (strcmp(name, "policy_configuration_outcome") == 0) ROUNDTRIP_REQUIRED(policy_configuration_outcome);
    else if (strcmp(name, "policy_dirty") == 0) ROUNDTRIP_REQUIRED(policy_dirty);
    else if (strcmp(name, "session_operation_request") == 0) ROUNDTRIP_REQUIRED(session_operation_request);
    else if (strcmp(name, "session_operation_outcome") == 0) ROUNDTRIP_REQUIRED(session_operation_outcome);
    else return 0;

    return status == SOPHIA_WM_V1_OK && encoded_len == frame_len &&
        memcmp(encoded, frame, frame_len) == 0;
}

static enum sophia_wm_v1_status reject_with(const char *decoder, const uint8_t *frame, size_t frame_len) {
    if (strcmp(decoder, "client_hello") == 0) {
        struct sophia_wm_v1_client_hello message;
        return sophia_wm_v1_decode_client_hello(frame, frame_len, &message);
    }
    if (strcmp(decoder, "server_welcome") == 0) {
        struct sophia_wm_v1_server_welcome message;
        return sophia_wm_v1_decode_server_welcome(frame, frame_len, &message);
    }
    if (strcmp(decoder, "snapshot_begin") == 0) {
        struct sophia_wm_v1_snapshot_begin message;
        uint64_t transaction = 0;
        return sophia_wm_v1_decode_snapshot_begin(frame, frame_len, &transaction, &message);
    }
    if (strcmp(decoder, "snapshot_chunk") == 0) {
        struct sophia_wm_v1_snapshot_chunk message;
        uint64_t transaction = 0;
        return sophia_wm_v1_decode_snapshot_chunk(frame, frame_len, &transaction, &message);
    }
    return SOPHIA_WM_V1_OK;
}

static const char *status_name(enum sophia_wm_v1_status status) {
    switch (status) {
        case SOPHIA_WM_V1_TRUNCATED: return "truncated";
        case SOPHIA_WM_V1_BAD_MAGIC: return "bad_magic";
        case SOPHIA_WM_V1_UNSUPPORTED_FRAME_VERSION: return "unsupported_frame_version";
        case SOPHIA_WM_V1_WRONG_MESSAGE_KIND: return "wrong_message_kind";
        case SOPHIA_WM_V1_PAYLOAD_TOO_LARGE: return "payload_too_large";
        case SOPHIA_WM_V1_RESERVED_NONZERO: return "reserved_nonzero";
        case SOPHIA_WM_V1_TRAILING_BYTES: return "trailing_bytes";
        case SOPHIA_WM_V1_INVALID_TRANSACTION: return "invalid_transaction";
        case SOPHIA_WM_V1_FIELD_TOO_LARGE: return "field_too_large";
        case SOPHIA_WM_V1_OK: return "ok";
    }
    return "unknown";
}

static int check_valid(const char *path) {
    FILE *input = fopen(path, "r");
    if (input == NULL) return 0;
    char line[FRAME_CAPACITY * 2u + 128u];
    uint8_t frame[FRAME_CAPACITY];
    size_t checked = 0;
    while (fgets(line, sizeof(line), input) != NULL) {
        line[strcspn(line, "\r\n")] = '\0';
        if (line[0] == '\0' || line[0] == '#') continue;
        char *cursor = line;
        char *name = next_field(&cursor);
        char *transaction_text = next_field(&cursor);
        char *hex = next_field(&cursor);
        if (cursor != NULL || transaction_text == NULL || hex == NULL) return 0;
        uint64_t transaction = strtoull(transaction_text, NULL, 10);
        size_t frame_len = decode_hex(hex, frame, sizeof(frame));
        if (frame_len == 0 || !roundtrip(name, transaction, frame, frame_len)) return 0;
        ++checked;
    }
    fclose(input);
    return checked == 15;
}

static int check_malformed(const char *path) {
    FILE *input = fopen(path, "r");
    if (input == NULL) return 0;
    char line[FRAME_CAPACITY * 2u + 128u];
    uint8_t frame[FRAME_CAPACITY];
    size_t checked = 0;
    while (fgets(line, sizeof(line), input) != NULL) {
        line[strcspn(line, "\r\n")] = '\0';
        if (line[0] == '\0' || line[0] == '#') continue;
        char *cursor = line;
        char *case_name = next_field(&cursor);
        char *decoder = next_field(&cursor);
        char *expected = next_field(&cursor);
        char *hex = next_field(&cursor);
        if (cursor != NULL || decoder == NULL || expected == NULL || hex == NULL) return 0;
        size_t frame_len = decode_hex(hex, frame, sizeof(frame));
        enum sophia_wm_v1_status status = reject_with(decoder, frame, frame_len);
        if (strcmp(status_name(status), expected) != 0) {
            fprintf(stderr, "%s: expected %s, got %s\n", case_name, expected, status_name(status));
            return 0;
        }
        ++checked;
    }
    fclose(input);
    return checked == 11;
}

#define ROUNDTRIP_RECORD(name, constant) do { \
    struct sophia_wm_v1_##name##_record record; \
    status = sophia_wm_v1_decode_##name##_record(data, data_len, 0, &record); \
    if (status == SOPHIA_WM_V1_OK) \
        status = sophia_wm_v1_encode_##name##_record(&record, encoded, sizeof(encoded)); \
    encoded_len = constant; \
} while (0)

static int record_roundtrip(const char *name, const uint8_t *data, size_t data_len) {
    uint8_t encoded[128];
    size_t encoded_len = 0;
    enum sophia_wm_v1_status status = SOPHIA_WM_V1_WRONG_MESSAGE_KIND;
    if (strcmp(name, "snapshot_output") == 0)
        ROUNDTRIP_RECORD(snapshot_output, SOPHIA_WM_V1_SNAPSHOT_OUTPUT_RECORD_SIZE);
    else if (strcmp(name, "snapshot_surface") == 0)
        ROUNDTRIP_RECORD(snapshot_surface, SOPHIA_WM_V1_SNAPSHOT_SURFACE_RECORD_SIZE);
    else if (strcmp(name, "snapshot_binding") == 0)
        ROUNDTRIP_RECORD(snapshot_binding, SOPHIA_WM_V1_SNAPSHOT_BINDING_RECORD_SIZE);
    else if (strcmp(name, "snapshot_session_operation") == 0)
        ROUNDTRIP_RECORD(snapshot_session_operation, SOPHIA_WM_V1_SNAPSHOT_SESSION_OPERATION_RECORD_SIZE);
    else if (strcmp(name, "projection_output") == 0)
        ROUNDTRIP_RECORD(projection_output, SOPHIA_WM_V1_PROJECTION_OUTPUT_RECORD_SIZE);
    else if (strcmp(name, "projection_placement") == 0)
        ROUNDTRIP_RECORD(projection_placement, SOPHIA_WM_V1_PROJECTION_PLACEMENT_RECORD_SIZE);
    else if (strcmp(name, "projection_indicator") == 0)
        ROUNDTRIP_RECORD(projection_indicator, SOPHIA_WM_V1_PROJECTION_INDICATOR_RECORD_SIZE);
    else if (strcmp(name, "projection_output_status") == 0)
        ROUNDTRIP_RECORD(projection_output_status, SOPHIA_WM_V1_PROJECTION_OUTPUT_STATUS_RECORD_SIZE);
    else return 0;
    return status == SOPHIA_WM_V1_OK && encoded_len == data_len &&
        memcmp(encoded, data, data_len) == 0;
}

static int check_records(const char *path) {
    FILE *input = fopen(path, "r");
    if (input == NULL) return 0;
    char line[512];
    uint8_t data[128];
    size_t checked = 0;
    while (fgets(line, sizeof(line), input) != NULL) {
        line[strcspn(line, "\r\n")] = '\0';
        if (line[0] == '\0' || line[0] == '#') continue;
        char *cursor = line;
        char *name = next_field(&cursor);
        char *hex = next_field(&cursor);
        if (cursor != NULL || hex == NULL) return 0;
        size_t data_len = decode_hex(hex, data, sizeof(data));
        if (data_len == 0 || !record_roundtrip(name, data, data_len)) return 0;
        ++checked;
    }
    fclose(input);
    return checked == 8;
}

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    if (!check_valid(argv[1]) || !check_malformed(argv[2]) || !check_records(argv[3])) return 1;
    return 0;
}
