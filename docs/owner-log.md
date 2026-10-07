# ingot fork — owner log

Persistent memory of the datamodel-owner agent for this fork. Read first, update before and after
every task. Upstream: brandon-arrendondo/ingot; this fork is interim pending upstream PR #1. Never
push, never open or update a PR (both are the operator's).

## Standing rules

- Changes are ADDITIVE templates, rebase-able onto upstream.
- ingot depends on nothing consumer-specific: it emits neutral, generic shapes a consumer's
  composition root instantiates.
- Every template has its own test through `cargo test`; a template that emits C++ also has a
  golden-output test.

## Gates (standalone, from the fork root)

| gate | command |
|---|---|
| Rust tests | `cargo test` |
| all hooks (fmt, clippy, llvm-cov 80%) | `pre-commit run --all-files` |
| C / C++ compile-and-run of generated output | `invoke test` |

## Tasks

### dm_event_list typelist (eventing spine E4 step 2) — branch `feat/dm-event-list`

- [x] `templates/dm_key_event_list.hpp`, rendered from the same `events` as the event structs.
- [x] `tests/codegen_event_list.rs` (golden + agreement + two emission gates), red first; five
      mutations caught.
- [x] C++ fixture instantiates the alias against real tinyfsm.
- [x] Superproject `inv build.host` with the regenerated gen/.
- Next (consumer, not ingot): the composition root instantiates
  `dm_event_list<b12::events::KeyEvent, b12::events::KeyList>`.
