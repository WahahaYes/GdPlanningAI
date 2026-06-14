# Campfire Timeout — Callback Resume Investigation

## Date
2025-06-14

## Summary
After implementing fixes for `pos`-shifting and caching of custom validity checks, the campfire tests still time out. This note records the isolation experiment and the new critical finding: **the planner stalls after 2 resumes, then runs for ~14 seconds before returning `success=false`**, suggesting the search is stuck in a long/infinite loop rather than waiting on async callbacks.

## Experiment: Isolate Campfire Tests

### Method
Run only `test_campfire_example_smoke.gd` with GUT command line, piping to `campfire_2000.log`:

```bash
godot --headless -s --path . addons/gut/gut_cmdln.gd \
  -gtest=res://test/integration/test_campfire_example_smoke.gd \
  -gdir=res://does_not_exist -ginclude_subdirs=false -gexit
```

Default timeout temporarily bumped from 300 → 2000 frames to see if more budget helps.

### Results
- **Total "Resuming search" logs**: 22 (across all 6 campfire test methods)
- **3/6 tests pass** — the 3 passing ones return `success=true` or `success=false` quickly (11–40 ms)
- **3/6 tests fail** — the failing ones show the same pattern: 2–3 resumes, then a 13+ second gap, then `success=false`

### Per-test breakdown from `campfire_2000.log`

| Test | Resumes | Time to complete | Result |
|------|--------:|------------------|--------|
| `test_full_cooking_chain` | 2 | 13,875.6 ms | timeout → `success=false` |
| `test_fire_too_low_to_cook` | 5 | 40.7 ms | `success=false` (expected) |
| `test_preemptive_fire_maintenance` | 3 | 13,927.6 ms | timeout → `success=false` |
| `test_competing_priorities_hunger_wins` | 2 | 13,907.4 ms | timeout → `success=false` |
| later passing tests | 2 each | 11–16 ms | `success=true, actions=0` |

### Key Observation
The **2nd resume** for the failing tests happens at around t=0.005 s, then there is **radio silence for ~13.9 s** until the test times out and the engine finally logs `Plan complete: success=false`.

This means the engine is **running continuously on the rayon thread for 13+ seconds** — it is NOT yielding for more callbacks. The iteration budget (`iteration_budget = 20000`) should have stopped it much earlier. Something inside `step_search` is consuming all that time.

## Current Hypotheses

1. **Infinite loop inside `step_search`**: The `while let Some(node) = self.queue.pop()` loop may be running without properly incrementing `iterations` or without the budget check firing, possibly because `iterations` is a local variable that gets reset or because the queue keeps refilling faster than it drains.

2. **Massive queue explosion**: Each resume creates new branches that each fire more pending callbacks. If parked nodes get re-queued with incorrect responses, the queue could grow exponentially. With `iteration_budget=20000`, the engine would churn through 20k iterations before yielding, which at ~1 µs/iteration would be 20 ms, not 14 seconds. But if each iteration fires a GDScript callback (synchronously inside the rayon thread!), the cost could be much higher.

3. **Synchronous callback dispatch inside `step_search`**: When `find_candidates` fires a `Pending` callback, it sends the request through `request_tx`. But the callback response comes back through `response_rx` which is read at the START of `step_search`. If the callback handler (in `simulation.rs`) is making synchronous Godot calls that block for milliseconds each, 20,000 iterations × 1ms = 20 seconds.

## Next Steps
- Verify whether callbacks inside `simulate_action` / `eval_precondition` are actually synchronous Godot calls (they invoke `Callable::call` directly, which blocks).
- Check if the iteration budget is actually being respected (add a hard panic/abort if iterations exceed 2× budget).
- Profile or instrument `step_search` to count how many loop iterations occur per resume and where time is spent.
