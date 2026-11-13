/* Drives the generated SDKs of the generic 1.1 model (sku_a) against the recording runtime:
 * wire encodings, length checks, buffers, struct writes, command submits, init and identity.
 * Built by tests/contract_sdk.rs. */
#include <string.h>

#include "sdk_inproc.h"
#include "sdk_integrator.h"
#include "sdk_app_cloud.h"
#include "sdk_app_map.h"
#include "store_fake.h"

static store_client_t client;
static uint64_t cs;

/**
 * @brief   Whether the last stored value is exactly `n` bytes `want`.
 * @version 1
 */
static int stored(const void *want, uint32_t n)
{
    return store_fake.last != NULL && store_fake.last->len == n && memcmp(store_fake.last->bytes, want, n) == 0;
}

/**
 * @brief   Whether the call log is `want`, then clear it.
 * @version 1
 */
static int logged(const char *want)
{
    int same = strcmp(store_fake.log, want) == 0;
    store_fake.log[0] = '\0';
    return same;
}

/**
 * @brief   Count scalar encoding failures: each type's wire bytes and round trip.
 * @version 1
 */
static int scalar_failures(void)
{
    int bad = 0;
    uint32_t odo = 0u;
    int16_t speed = 0;
    bool on = false;
    float gain = 0.0f;
    const uint8_t odo_bytes[] = { 0x78u, 0x56u, 0x34u, 0x12u };
    const uint8_t speed_bytes[] = { 0xFEu, 0xFFu };
    const uint8_t one = 1u;
    const float f = 1.5f;
    uint8_t f_bytes[4];
    uint32_t u;

    bad += sdk_integrator_set_drive_state_odometer(&client, 0x12345678u, &cs) != STORE_OK;
    bad += !stored(odo_bytes, 4u) || cs != 9u;
    bad += sdk_integrator_get_drive_state_odometer(&client, &odo, NULL) != STORE_OK || odo != 0x12345678u;
    bad += sdk_integrator_set_drive_state_speed(&client, -2, &cs) != STORE_OK || !stored(speed_bytes, 2u);
    bad += sdk_integrator_get_drive_state_speed(&client, &speed, NULL) != STORE_OK || speed != -2;
    bad += sdk_integrator_batch_add_drive_state_enabled(&client, true) != STORE_OK || !stored(&one, 1u);
    bad += sdk_integrator_get_drive_state_enabled(&client, &on, NULL) != STORE_OK || !on;
    memcpy(&u, &f, 4u);
    for (int i = 0; i < 4; i++) {
        f_bytes[i] = (uint8_t)(u >> (8 * i));
    }
    bad += sdk_app_cloud_set_drive_settings_gain(&client, 1.5f, &cs) != STORE_OK || !stored(f_bytes, 4u);
    bad += sdk_integrator_get_drive_settings_gain(&client, &gain, NULL) != STORE_OK || gain != 1.5f;
    return bad;
}

/**
 * @brief   Count length and buffer failures.
 * @version 1
 */
static int buffer_failures(void)
{
    int bad = 0;
    uint32_t odo = 0u;
    uint64_t seq = 0u;
    char label[32];
    uint32_t len = 0u;

    bad += sdk_integrator_set_drive_state_odometer(&client, 1u, &cs) != STORE_OK;
    store_fake.last->len = 3u;                      /* a stored value of the wrong length */
    bad += sdk_integrator_get_drive_state_odometer(&client, &odo, &seq) != STORE_E_TYPE || seq != 7u;
    bad += sdk_integrator_set_drive_state_label(&client, "idle", 4u, &cs) != STORE_OK || !stored("idle", 4u);
    bad += sdk_integrator_get_drive_state_label(&client, label, sizeof label, &len, NULL) != STORE_OK;
    bad += len != 4u || memcmp(label, "idle", 4u) != 0;
    bad += sdk_integrator_set_drive_state_label(&client, label, 33u, &cs) != STORE_E_TOOBIG;
    return bad;
}

/**
 * @brief   Count command and struct failures.
 * @version 1
 */
static int command_failures(void)
{
    int bad = 0;
    const store_cmd_id_t id = { 0x01020304u, 5u };
    store_cmd_origin_t origin;
    const uint8_t payload[] = { 0xAAu, 0xBBu };
    const uint8_t value[] = { 4u, 3u, 2u, 1u, 5u, 0u, 0u, 0u, 0xAAu, 0xBBu };
    uint8_t big[65];

    memset(&origin, 0, sizeof origin);
    origin.source = 3u;
    store_fake.log[0] = '\0';
    bad += sdk_app_cloud_submit_drive_command_drive(&client, &origin, id, payload, 2u, &cs) != STORE_OK;
    bad += !logged("cmd_submit ") || !stored(value, 10u) || store_fake.origin.source != 3u || cs != 13u;
    bad += sdk_app_cloud_submit_drive_command_drive(&client, NULL, id, payload, 2u, &cs) != STORE_OK;
    bad += !logged("set ") || !stored(value, 10u);
    bad += sdk_app_cloud_submit_drive_command_drive(&client, &origin, id, big, 65u, &cs) != STORE_E_TOOBIG;
    bad += !logged("");
    bad += sdk_app_cloud_submit_drive_command_map_edit(&client, &origin, id, payload, 2u, &cs) != STORE_OK;
    bad += !logged("alloc data struct_submit ") || store_fake.struct_len != 10u;
    bad += memcmp(store_fake.struct_bytes, value, 10u) != 0 || cs != 12u;
    bad += sdk_app_map_set_drive_map_map_data(&client, payload, 2u, &cs) != STORE_OK;
    bad += !logged("alloc data publish ") || store_fake.struct_len != 2u || cs != 11u;
    return bad;
}

/**
 * @brief   Count init and identity failures.
 * @version 1
 */
static int identity_failures(void)
{
    int bad = 0;

    bad += sdk_inproc_init(&client, NULL) != STORE_OK;
    bad += store_fake.cfg.transport != STORE_TRANSPORT_INPROC || store_fake.cfg.principal_id != 10u;
    bad += store_fake.cfg.struct_size != sizeof(store_client_cfg_t);
    bad += sdk_integrator_init(&client, "/run/ks.sock") != STORE_OK;
    bad += store_fake.cfg.transport != STORE_TRANSPORT_UNIX || strcmp(store_fake.cfg.endpoint, "/run/ks.sock") != 0;
    bad += sdk_integrator_connect(&client, 42u) != STORE_OK || store_fake.id.token != 42u;
    bad += store_fake.id.contract_major != 1u || store_fake.id.contract_minor != 1u || store_fake.id.keyset_len == 0u;
    for (uint32_t i = 1u; i < store_fake.id.keyset_len; i++) {
        bad += store_fake.id.keyset_id[i - 1u] >= store_fake.id.keyset_id[i];
        bad += store_fake.id.keyset_type[i] != ((store_fake.id.keyset_id[i] >> 3) & 0xFu);
    }
    bad += memcmp(store_fake.id.model_hash, SDK_MODEL_HASH, STORE_MODEL_HASH_CHARS) != 0;
    return bad;
}

/**
 * @brief   Run every check.
 * @version 1
 */
int main(void)
{
    int bad = scalar_failures() + buffer_failures() + command_failures() + identity_failures();
    return bad != 0;
}
