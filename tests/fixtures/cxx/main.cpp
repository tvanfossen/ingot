/* Test fixture driver — proves the generated C++ event artifacts compile,
 * link, and resolve the consumer dispatch seam against real tinyfsm.
 *
 * Compiling dm_key_events_wrapper.cpp instantiates send_tinyfsm_event<E>
 * for every FSM_EVENT_* struct (one per event key), so a successful build
 * already exercises every switch case. main() then links and runs the
 * dispatch entry point to confirm the seam is satisfied end-to-end.
 *
 * dm_key_has_event() is checked at run time against examples/full.toml, the
 * model `invoke test` generates this fixture from: an event key must answer
 * true, a non-event key and an id that is no key must answer false. */
#include "tinyfsm.hpp"
#include "fsmlist.hpp"
#include "dm_key_events_wrapper.hpp"
#include "dm_key_has_event.hpp"

FSM_INITIAL_STATE(DummyFsm, DummyFsm)

/**
 * @brief   Check dm_key_has_event() on one key of each kind from examples/full.toml.
 * @version 1
 * @return  0 when every answer is right, 1 otherwise.
 */
static int check_has_event(void)
{
    int failures = 0;
    if (!dm_key_has_event(DM_KEY_APPLIANCE_STATUS_MODE)) {
        failures++; /* event = true */
    }
    if (dm_key_has_event(DM_KEY_APPLIANCE_STATUS_TEMPERATURE)) {
        failures++; /* event = false */
    }
    if (dm_key_has_event(0U)) {
        failures++; /* not a key of this model */
    }
    return (failures == 0) ? 0 : 1;
}

/**
 * @brief   Start the FSM list, drive the dispatch seam once, then check the predicate.
 * @version 2
 * @return  process exit status: 0 on success.
 */
int main(void)
{
    fsm_list::start();
    /* key_id 0 falls through to the default branch — model-agnostic, just
     * forces the wrapper to link. The switch cases are already compiled. */
    send_tinyfsm_event_by_key(0U);
    return check_has_event();
}
