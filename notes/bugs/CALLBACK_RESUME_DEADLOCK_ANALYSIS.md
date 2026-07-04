# Callback Resume Deadlock Analysis

**Date:** 2025-06-14
**Status:** Active investigation

## Summary

After implementing `expanded_candidates` to prevent duplicate candidate expansion, the `test_hunger_example_smoke.gd` shake-tree phase now passes (GoTo -> Shake Tree found in ~25ms with 9 resumes). However, the pickup phase and all campfire tests still time out.

## Key Observation: Very Few Resumes

The most important signal is the extremely low "Resuming search" count during timeouts:

| Test | Resumes | Elapsed Time |
|------|---------|-------------|
| Hunger shake tree (passes) | 9 | ~25ms |
| Hunger pickup (times out) | 5 | ~1.2s |
| Campfire full cooking chain (times out) | 2 | ~14s |
| Campfire preemptive fire (times out) | 3 | ~14s |

With `iteration_budget = 20000`, the engine should yield frequently. The fact that there are only 2-5 resumes over thousands of frames means the **scheduler is not resuming the engine**.

## Scheduler Resume Logic

In `scheduler.rs` step 3:

```rust
let ready_to_resume = if job.pending_request_id == 0 {
    true  // budget yield -> always resume
} else {
    jobs_with_responses.contains(&job.agent_instance_id)
};
```

`jobs_with_responses` is populated in step 2 by processing requests from `job.request_rx`. If **no requests are in `request_rx`**, `jobs_with_responses` is empty, and the engine is **never resumed**.

## The Deadlock Cycle

The engine returns `PlannerRunResult::Pending(id)` when `step_search` encounters a pending callback. The scheduler stores this `id` in `job.pending_request_id`. On the next `process_callbacks()`, it processes any requests in `request_rx` and resumes the engine if responses were sent.

The deadlock occurs when:
1. Engine returns `Pending(id)` where `id` is from `parked_nodes.keys().next().unwrap()`
2. The scheduler does NOT find any requests in `request_rx` for this job
3. `jobs_with_responses` remains empty
4. `ready_to_resume` is `false`
5. Engine is never resumed again
6. Test times out

## Root Cause Hypothesis: Stale `parked_nodes` or `discovery_precond_pending`

`find_candidates` evaluates actions and may return `StepResult::Pending(id)` when it finds an entry in `discovery_precond_pending` or `discovery_pending`. If:

- A response for `id` was **already processed** in a previous `step_search` run
- But `parked_nodes` still contains nodes parked on `id`
- OR `discovery_precond_pending` still contains a stale entry for `id`

Then `find_candidates` returns `Pending(id)`, but **no new callback request is sent** (the response was already processed). `request_rx` is empty. The scheduler never resumes.

## Potential Stale Entry Sources

1. **Key mismatch in `discovery_precond_pending`**: `find_candidates` inserts with key `(idx, pre.clone(), bindings.clone())`. `engine.rs` removes with `(idx, spec, bindings)` where `spec` comes from `DiscoveryRequest`. If `Eq`/`Hash` behaves differently for borrowed vs owned keys, `remove` might silently fail.

2. **Race between `find_candidates` and response processing**: Though both run on the same Rayon thread, `find_candidates` acquires `discovery_precond_pending` lock, reads an entry, and returns `Pending(id)`. If the response was processed earlier in the same `step_search` but `find_candidates` is called for a different node after the lock is released, it might see a stale entry.

3. **`parked_nodes` accumulation**: Nodes parked on old `id`s might not be cleaned up if `step_search` returns `Pending(id)` before fully processing responses (e.g., due to `cancel_flag` check inside the response loop).

## Why Shake Tree Passes

The shake tree phase has only 4 actions and a shallow plan (depth 2). All pending callbacks are resolved quickly, and the search completes before any stale entry situation can arise.

Complex scenarios (campfire with 15 actions, depth up to 6) have many more pending callbacks. The serialized callback processing creates many resume cycles, increasing the chance of hitting a stale entry.

## Update After Second Test Run (with stale entry cleanup)

### Hunger Test
- Phase 3 (pickup/eat) now completes in ~32ms with 14 resumes and 19 branches.
- Plan found: `Go To -> Wander -> Pick Up Item -> Eat Held Food` (4 actions, cost 3.73)
- Test fails on plan quality (expects 3 actions, gets 4 with extra `Wander`) -- NOT a timeout.

### Campfire Tests (Still Deadlocked)
All three show the same deadlock pattern:

| Test | Resumes | Branches | Time |
|------|---------|----------|------|
| `test_full_cooking_chain` | **2** | 6 | **13805ms** |
| `test_preemptive_fire_maintenance` | **3** | 1 | **13825ms** |
| `test_competing_priorities_hunger_wins` | **2** | 6 | timeout |

Debug trees consistently show only `Eat Held Food` -> `Pick Up Wood` (x4). **`Cook Potato`, `Dig Potato`, `Go To` never appear.** These all have custom `check_is_object_valid` validity checks requiring callbacks.

### Key Observations
- No `"Search budget exceeded"` warnings. Engine returns `Pending(id)` well before 20,000 iterations.
- `Active Search Threads: 0` at timeout means the engine thread finished and returned a result, but the scheduler **never resumed it again**.
- `test_fire_too_low_to_cook` demonstrates the scheduler **can** resume 22 times when there are no candidates (fails immediately).

### The Deadlock
After 2-3 cycles, the engine returns `Pending(id)` but `request_rx` is empty, so `jobs_with_responses` stays empty and `ready_to_resume` is `false`.

Stale entry cleanup in `expander.rs` was applied to:
- `get_discovery_result` (simulation callbacks)
- Validity checks in `find_candidates`
- Open preconditions in `find_candidates`

But campfire tests still stall. This suggests either:
1. A code path returns `Pending(id)` **without** a matching callback request
2. `request_tx.send(...)` silently fails for some requests
3. The node is parked on an `id` that was never inserted into `discovery_request_map`

## Update After Third Test Run (with scheduler-side fix)

### Root Cause Found and Fixed

The deadlock was caused by a race between the engine's background thread and the scheduler's main thread:

1. Engine sends request `id` and parks a node
2. Scheduler processes request `id` and sends response
3. Engine was already past its `response_rx.try_recv()` loop at the start of `step_search`
4. Engine exhausts queue, returns `Pending(id)`
5. Scheduler sees `Pending(id)` but `request_rx` is empty (already processed)
6. Scheduler never resumes -> deadlock

**Fix**: The scheduler now tracks `completed_request_ids: HashSet<usize>` per job. When it sends a response, it records the request ID. When deciding whether to resume, it checks `completed_request_ids.contains(&job.pending_request_id)` in addition to `jobs_with_responses`. This ensures the engine is resumed even when the response was sent in a previous frame.

Additionally, re-applied the `has_ready_candidates` fix to prevent parking nodes when there are already ready candidates.

### Current Test Results

| Test Suite | Before Fix | After Fix |
|------------|-----------|-----------|
| `test_async_planner` | 10/10 passed | 10/10 passed |
| `test_campfire` | 3/6 passed (timeouts) | **5/6 passed** |
| `test_competing_priorities` | 0/1 passed (timeout) | 0/1 passed |
| `test_requirements_provisions` | 9/10 passed | **10/10 passed** |
| `test_search_tree` | 12/12 passed | 12/12 passed |
| `test_blackboard_clone` | 4/4 passed | 4/4 passed |

**Campfire timeouts are completely gone.** The scheduler now correctly resumes engines.

### Remaining Issues (Plan Quality, Not Timeouts)

1. **`test_full_cooking_chain`**: `Agent should plan when hungry with fire available, but got empty plan`
   - Plan returns success=false with 0 actions. Need to investigate why no valid chain is found.

2. **`test_go_to_shake_tree`**: `[1] expected to equal [2]: Plan should have GoTo → Shake Tree chain`
   - Got 1 action ("Wander") instead of 2 ("Go To", "Shake Tree"). The "Wander" action is being chosen instead of the required "Go To" → "Shake Tree" sequence.

3. **`test_competing_priorities_hunger_wins`**: Still timing out or returning empty plans.

These are no longer scheduler deadlocks. They are likely issues with:
- Action validity checks (`check_is_object_valid`)
- Requirement/provision matching
- Cost calculations causing suboptimal action selection
- Missing action chains in the search space
