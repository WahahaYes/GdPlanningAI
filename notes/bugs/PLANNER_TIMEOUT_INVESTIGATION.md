# Bug: Planner Search Stalls & Thread Leakage

**Status**: Investigating / Partially Fixed
**Severity**: High (Causes CI timeouts and inconsistent test results)

## Symptoms
- Integration tests (specifically campfire scenarios) time out after 2-5 seconds.
- Debug trees show very low branch counts (1-20) despite long execution times.
- `POOL STATUS` reveals `Active Jobs` count increasing monotonically across test runs.
- Search often terminates with `FAILURE — no plan found` even when valid chains exist.

## Root Causes Identified

### 1. Resurrection of Cancelled Jobs
In `scheduler.rs`, jobs that yielded for budget (`iterations > 20000`) were being re-spawned into the Rayon pool without checking if they had been cancelled while yielded.
- **Fix**: Added `cancel_flag` check in `process_callbacks` before `run_job_step`.

### 2. Async Callback Deadlock
If a test times out and the Godot-side test script finishes, any `Pending` discovery simulations for that job will never receive their responses. The Rust engine remains "parked" in the `parked_nodes` map forever.
- **Fix**: Added `cancel_all_jobs()` to the test `after_each` and improved scheduler reaping logic.

### 3. Causal Link Congestion
When multiple actions in a chain introduce specific state requirements (e.g., `at_target(PotatoPatch)` followed by `at_target(Campfire)`), the planner tries to satisfy both simultaneously in the `open_requirements` list.
- **Impact**: This creates impossible candidates or causes the search to declare failure because no single action (like `Go To`) can satisfy two distinct physical locations at once.

### 4. Hybrid Simulation Latency
Every branch expansion that requires a Godot callback (cost/effect simulation) adds ~16.6ms (one frame) of latency. In a complex chain, this creates a "conga line" effect where the planner spends 99% of its time waiting for the main thread.
- **Improvement**: Refactored `find_candidates` to return `Ready` candidates immediately while `Pending` ones are still in flight.

## Remaining Work
- [ ] Audit `engine.rs` to ensure `parked_nodes` are dropped immediately on cancellation.
- [ ] Implement sequential requirement handling to prevent `at_target` congestion.
- [ ] Investigate why `FAILURE` is declared when the queue is still theoretically deep.
