/* Links against a generated dsh_tables.c and walks it, the way the library's binder, reporter,
 * command router and error reporter would: every table the header declares must be defined,
 * sorted where the interface says so, and every index and value reference must be in range.
 * Built by tests/shadow_tables.rs. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "dsh_tables.h"

_Static_assert(DSH_IF_MAJOR == 1, "shadow table interface major");

/**
 * @brief   Whether a value reference lies inside dsh_value_blob.
 * @version 1
 * @return  1 when it does.
 */
static int value_ok(dsh_value_t v)
{
    return (size_t)v.off + v.len <= DSH_VALUE_BLOB_LEN;
}

/**
 * @brief   Whether `first .. first + count` lies inside a table of `total` rows.
 * @version 1
 * @return  1 when it does.
 */
static int slice_ok(size_t first, size_t count, size_t total)
{
    return first + count <= total;
}

/**
 * @brief   Whether a 16-bit index is DSH_NONE_U16 or below `total`.
 * @version 1
 * @return  1 when it is.
 */
static int index_ok(uint16_t i, size_t total)
{
    return i == DSH_NONE_U16 || i < total;
}

/**
 * @brief   Count violations in the enums: slices in range, outbound sorted by code and inbound
 *          by text, each strictly.
 * @version 1
 * @return  the number of violations.
 */
static int enum_violations(void)
{
    int bad = 0;
    size_t e, i;
    for (e = 0u; e < DSH_NUM_ENUMS; e++) {
        const dsh_enum_t *en = &dsh_enums[e];
        bad += !slice_ok(en->out_first, en->out_count, DSH_NUM_OUTBOUND);
        bad += !slice_ok(en->in_first, en->in_count, DSH_NUM_INBOUND);
        for (i = 1u; i < en->out_count; i++) {
            bad += dsh_outbound[en->out_first + i - 1u].code >= dsh_outbound[en->out_first + i].code;
        }
        for (i = 1u; i < en->in_count; i++) {
            bad += strcmp(dsh_inbound[en->in_first + i - 1u].text, dsh_inbound[en->in_first + i].text) >= 0;
        }
    }
    return bad;
}

/**
 * @brief   Whether `code` is an outbound code of enum `e`.
 * @version 1
 * @return  1 when it is.
 */
static int has_code(uint16_t e, int64_t code)
{
    size_t i;
    int found = 0;
    for (i = 0u; i < dsh_enums[e].out_count; i++) {
        found |= dsh_outbound[dsh_enums[e].out_first + i].code == code;
    }
    return found;
}

/**
 * @brief   Index of the desired row at `path` by binary search (as the binder does), or -1.
 * @version 1
 * @return  the row index, or -1.
 */
static long find_desired(const char *path)
{
    size_t lo = 0u;
    size_t hi = DSH_NUM_DESIRED;
    long found = -1;
    while (lo < hi && found < 0) {
        size_t mid = lo + (hi - lo) / 2u;
        int c = strcmp(dsh_desired[mid].path, path);
        if (c == 0) {
            found = (long)mid;
        } else if (c < 0) {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    return found;
}

/**
 * @brief   Count violations in one desired row.
 * @version 1
 * @return  the number of violations.
 */
static int desired_row_violations(size_t i)
{
    const dsh_desired_t *d = &dsh_desired[i];
    int bad = find_desired(d->path) != (long)i;
    size_t k;
    bad += d->key == DMT_ID_NONE || !index_ok(d->enum_index, DSH_NUM_ENUMS);
    bad += !slice_ok(d->allowed_first, d->allowed_count, DSH_NUM_ALLOWED);
    bad += !slice_ok(d->implies_first, d->implies_count, DSH_NUM_IMPLIES);
    bad += d->apply_group != DSH_NONE_U8 && d->apply_group >= DSH_NUM_APPLY_GROUPS;
    bad += d->form != DSH_FORM_PLAIN && d->enum_index == DSH_NONE_U16;
    bad += d->inbound_unknown == DSH_IN_CODE && !has_code(d->enum_index, d->inbound_unknown_code);
    bad += d->inbound_unknown != DSH_IN_NONE && d->form != DSH_FORM_ENUM;
    for (k = 0u; k < d->implies_count && slice_ok(d->implies_first, k + 1u, DSH_NUM_IMPLIES); k++) {
        bad += !value_ok(dsh_implies[d->implies_first + k].value);
    }
    return bad;
}

/**
 * @brief   Count violations in the binder's tables: desired and echo rows sorted and unique.
 * @version 1
 * @return  the number of violations.
 */
static int binder_violations(void)
{
    int bad = 0;
    size_t i;
    for (i = 0u; i < DSH_NUM_DESIRED; i++) {
        bad += i > 0u && strcmp(dsh_desired[i - 1u].path, dsh_desired[i].path) >= 0;
        bad += desired_row_violations(i);
    }
    for (i = 1u; i < DSH_NUM_ECHO; i++) {
        bad += strcmp(dsh_echo[i - 1u].path, dsh_echo[i].path) >= 0;
    }
    return bad;
}

/**
 * @brief   Number of leading object segments (each followed by a dot) two paths share.
 * @version 1
 * @return  the count.
 */
static size_t shared_objects(const char *a, const char *b)
{
    size_t n = 0u;
    size_t i = 0u;
    while (a[i] != '\0' && a[i] == b[i]) {
        n += a[i] == '.';
        i++;
    }
    return n;
}

/**
 * @brief   Count violations in one reported row's references.
 * @version 1
 * @return  the number of violations.
 */
static int reported_row_violations(const dsh_reported_t *r)
{
    int bad = r->scope == 0u || (r->scope & ~(DSH_SC_FULL | DSH_SC_DELTA)) != 0u;
    bad += !index_ok(r->enum_index, DSH_NUM_ENUMS);
    bad += r->kind == DSH_RK_KEY && r->key == DMT_ID_NONE;
    bad += r->kind == DSH_RK_CONSTANT && r->constant == NULL;
    bad += r->kind == DSH_RK_ECHO && r->ref >= DSH_NUM_ECHO;
    bad += r->outbound_unknown == DSH_OUT_WIRE && r->outbound_unknown_wire == NULL;
    bad += (r->flags & DSH_RF_FORCED) != 0u && (r->scope & DSH_SC_DELTA) == 0u;
    return bad;
}

/**
 * @brief   Count violations in the reporter's rows: references in range, and rows sharing an
 *          object prefix contiguous (a prefix left is never re-entered).
 * @version 1
 * @return  the number of violations.
 */
static int reporter_violations(void)
{
    int bad = 0;
    size_t i, j, k;
    for (i = 0u; i < DSH_NUM_REPORTED; i++) {
        bad += reported_row_violations(&dsh_reported[i]);
        for (j = i + 1u; j < DSH_NUM_REPORTED; j++) {
            for (k = j + 1u; k < DSH_NUM_REPORTED; k++) {
                size_t ik = shared_objects(dsh_reported[i].path, dsh_reported[k].path);
                bad += ik > shared_objects(dsh_reported[i].path, dsh_reported[j].path);
            }
        }
    }
    return bad;
}

/**
 * @brief   Count violations in the error groups: bits in range, sorted, below the width and
 *          never reserved.
 * @version 1
 * @return  the number of violations.
 */
static int error_violations(void)
{
    int bad = 0;
    size_t g, i;
    for (g = 0u; g < DSH_NUM_ERROR_GROUPS; g++) {
        const dsh_error_group_t *eg = &dsh_error_groups[g];
        bad += !slice_ok(eg->bits_first, eg->bits_count, DSH_NUM_ERROR_BITS);
        for (i = 0u; i < eg->bits_count; i++) {
            const dsh_error_bit_t *b = &dsh_error_bits[eg->bits_first + i];
            bad += b->bit >= eg->width || ((eg->reserved >> b->bit) & 1u) != 0u;
            bad += i > 0u && dsh_error_bits[eg->bits_first + i - 1u].bit >= b->bit;
        }
    }
    return bad;
}

/**
 * @brief   Count violations in one step's arguments.
 * @version 1
 * @return  the number of violations.
 */
static int step_violations(const dsh_step_t *s)
{
    int bad = !slice_ok(s->args_first, s->args_count, DSH_NUM_ARGS);
    size_t i;
    for (i = 0u; i < s->args_count && slice_ok(s->args_first, i + 1u, DSH_NUM_ARGS); i++) {
        const dsh_arg_t *a = &dsh_args[s->args_first + i];
        bad += !value_ok(a->value) || a->key == DMT_ID_NONE;
        bad += ((a->flags & DSH_AF_CONSTANT) != 0u) != (a->field == NULL);
        bad += !index_ok(a->enum_index, DSH_NUM_ENUMS);
    }
    return bad;
}

/**
 * @brief   Count violations in the command rows: a sub topic, steps exactly for the steps action.
 * @version 1
 * @return  the number of violations.
 */
static int command_violations(void)
{
    int bad = 0;
    size_t c, i;
    for (c = 0u; c < DSH_NUM_COMMANDS; c++) {
        const dsh_command_t *r = &dsh_commands[c];
        bad += r->topic >= DTP_NUM_TOPICS || dtp_topics[r->topic].direction != DTP_SUB;
        bad += !slice_ok(r->steps_first, r->steps_count, DSH_NUM_STEPS);
        bad += (r->action == DSH_DO_STEPS) != (r->steps_count > 0u);
        bad += r->action == DSH_DO_INTERNAL && r->internal == NULL;
        bad += r->body == DSH_BODY_OBJECT && r->body_object == NULL;
        for (i = 0u; i < r->steps_count && slice_ok(r->steps_first, i + 1u, DSH_NUM_STEPS); i++) {
            bad += step_violations(&dsh_steps[r->steps_first + i]);
        }
    }
    return bad;
}

/**
 * @brief   Count violations in the topics: an ack names a pub topic, a source enum exists and
 *          holds its absent code, an envelope flag goes with an envelope field.
 * @version 1
 * @return  the number of violations.
 */
static int topic_violations(void)
{
    int bad = 0;
    size_t t;
    for (t = 0u; t < DTP_NUM_TOPICS; t++) {
        const dtp_topic_t *p = &dtp_topics[t];
        bad += p->ack != DSH_NONE_U8 && (p->ack >= DTP_NUM_TOPICS || dtp_topics[p->ack].direction != DTP_PUB);
        bad += !index_ok(p->source_enum, DSH_NUM_ENUMS);
        bad += p->source_enum != DSH_NONE_U16 && !has_code(p->source_enum, p->source_absent);
        bad += (p->flags & DTP_F_ID_REQUIRED) != 0u && p->id_field == NULL;
        bad += !slice_ok(p->payload_first, p->payload_count, DTP_NUM_PAYLOAD);
        bad += p->payload_count > 0u && p->direction != DTP_PUB;
    }
    return bad;
}

/**
 * @brief   Walk every table.
 * @version 1
 * @return  0 when the tables are consistent.
 */
int main(void)
{
    int bad = enum_violations() + binder_violations() + reporter_violations();
    bad += error_violations() + command_violations() + topic_violations();
    bad += dsh_if_major_1 != 1u;
    return bad != 0;
}
