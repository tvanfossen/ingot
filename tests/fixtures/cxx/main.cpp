/* Test fixture driver — proves the generated C++ event artifacts compile,
 * link, and resolve the consumer dispatch seam against real tinyfsm.
 *
 * Compiling dm_key_events_wrapper.cpp instantiates send_tinyfsm_event<E>
 * for every FSM_EVENT_* struct (one per event key), so a successful build
 * already exercises every switch case. main() then links and runs the
 * dispatch entry point to confirm the seam is satisfied end-to-end.
 *
 * dm_event_list is instantiated with consumer-shaped KeyEvent / KeyList
 * templates defined here, the way a composition root instantiates it with its
 * own: every entry must name a real key id and a tinyfsm event type. */
#include <cstddef>
#include <cstdint>
#include <type_traits>

#include "tinyfsm.hpp"
#include "fsmlist.hpp"
#include "dm_key_events_wrapper.hpp"
#include "dm_key_event_list.hpp"

FSM_INITIAL_STATE(DummyFsm, DummyFsm)

/* A consumer's (key id, event type) pair. */
template<std::uint32_t Id, class E>
struct KeyEvent
{
    static constexpr std::uint32_t id = Id;
    using event = E;
};

/* A consumer's list: counts its entries and checks each is a tinyfsm event. */
template<class... KE>
struct KeyList;

template<>
struct KeyList<>
{
    static constexpr std::size_t size = 0U;
    static constexpr bool all_events = true;
};

template<class Head, class... Tail>
struct KeyList<Head, Tail...>
{
    static constexpr std::size_t size = 1U + KeyList<Tail...>::size;
    static constexpr bool all_events =
        std::is_base_of<tinyfsm::Event, typename Head::event>::value && KeyList<Tail...>::all_events;
};

using Keys = dm_event_list<KeyEvent, KeyList>;
static_assert(Keys::size == dm_event_count, "dm_event_list size must equal dm_event_count");
static_assert(Keys::size > 0U, "fixture model must carry event keys");
static_assert(Keys::all_events, "every dm_event_list entry must name a tinyfsm event");

int main(void)
{
    fsm_list::start();
    /* key_id 0 falls through to the default branch — model-agnostic, just
     * forces the wrapper to link. The switch cases are already compiled. */
    send_tinyfsm_event_by_key(0U);
    return 0;
}
