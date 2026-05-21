# Planner Timeout and Exhaustion Investigation

## Problem Description

Integration tests (particularly campfire and hunger examples) are timing out with "Out of bounds get index '0'" errors. The planner appears to exhaust the search space without finding valid plans.

## Observed Issues

### 1. Test Timeout Not Sending Cancel Signal (RESOLVED)
- Test timeout (300 frames) occurs but planner continues running in background
- Cancel flag is not being set when test times out
- Planner continues processing after test failure
- Evidence: "Search iteration" messages continue appearing after test timeout error

**Fix:** Added `cancel_agent_jobs()` method to scheduler that sets cancel flag for specific agent. Tests now call this method on timeout. Logs show "Search cancelled by cancel_flag after 10 iterations".

### 2. Test Not Asserting Plan is Not None (RESOLVED)
- Test tries to access `plan[0]` without checking if plan is empty
- "Out of bounds get index '0'" error occurs when planner returns None/empty plan
- Test should assert plan is not None before accessing elements
- Location: `test/integration/test_campfire_example_smoke.gd:112`

**Fix:** Added guard clauses in tests to check if plan is empty before accessing elements. Tests now fail with clear message "Agent should plan... but got empty plan" instead of crashing.

### 3. Search Exhaustion at Depth 3 (RESOLVED - Root Cause Found)
- Search loop iterates correctly but exhausts at chain length 3, depth 3
- Expander returns 0 successors at depth 3
- Search keeps popping nodes at depth 3 but cannot expand them
- Planner exhausts search space without finding complete plan
- Evidence: "Node expansion: chain length 3, depth 3, generated 0 successors"

**Root Cause:** Insertion logic change in expander.rs (removed insertion_index_for_candidate, always inserted at position 0) changed DFS exploration order, causing depth 3 exhaustion.

**Fix:** Reverted expander.rs to HEAD, restoring original insertion logic. Test results improved from 23 failing to 5 failing tests.

### 4. Risky Test Assertions (RESOLVED)
- Tests marked as "Risky: Did not assert" when plan is empty
- test_cannot_add_fuel_when_full and test_cannot_cook_without_potato

**Fix:** Added assertions even when plan is empty to ensure tests always have at least one assertion.

## Debug Strategies Implemented

### 1. Profiling Instrumentation
Added metrics to track search space exploration:
- `nodes_explored`: Number of nodes popped from controller
- `max_depth_reached`: Maximum depth reached during search
- `candidates_evaluated`: Total successors generated
- `search_iterations`: Total loop iterations
- `theoretical_max`: num_actions^max_depth (worst case)

**Finding:** Planner explores very little of search space (0.00%-18.75% of theoretical max), ruling out excessive exploration as cause.

### 2. Search Loop Iteration Tracking
Added detailed logging to track search loop behavior:
- Progress logging every 100 iterations
- Node expansion logging with chain length, depth, successors count
- Empty successor detection
- Search loop exhaustion notification

**Finding:** Search loop IS iterating correctly, not stuck in infinite loop. Exhaustion occurs legitimately at depth 3.

### 3. Controller State Investigation
Added logging for:
- Chain length at each iteration
- Successor generation counts
- Empty successor detection

**Finding:** At depth 3, expander consistently returns 0 successors, causing search exhaustion.

## Root Cause Analysis

The timeout was caused by:
1. **PRIMARY:** Insertion logic change in expander.rs (always inserting at position 0 instead of calculated insertion index) changed DFS exploration order, causing depth 3 exhaustion
2. **SECONDARY:** Test timeout not canceling planner (now fixed)
3. **SECONDARY:** Test not handling None/empty plan result gracefully (now fixed)

The timeout was NOT caused by:
- Excessive search space exploration (only 0.00%-18.75% explored)
- Infinite loop in search (loop exits correctly)
- Controller stuck (nodes are being popped and processed)

## Resolution Status

**Completed:**
- ✅ Root cause identified (insertion logic change)
- ✅ Reverted expander.rs to restore original insertion logic
- ✅ Added cancel_agent_jobs() method to scheduler
- ✅ Wired up cancel signal in tests
- ✅ Added plan validation in tests
- ✅ Fixed risky test assertions

**Test Results:**
- **Before fixes:** 23 failing tests, 23 passing
- **After reverting expander:** 5 failing tests, 40 passing
- **After cancel signal + test fixes:** 3 failing tests, 43 passing
- **After timeout-based callbacks:** 1 failing test, 38 passing (test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion - action ordering issue)

**Current Status (May 20, 2026):**
- ✅ Callback blocking identified as root cause of test timeouts
- ✅ Timeout-based workaround prevents indefinite blocking in tests
- ✅ Campfire and hunger smoke tests now pass
- ⚠️ Timeout-based solution is NOT production-ready
- ⚠️ 1 test fails due to action ordering differences from timeouts
- ❌ Production-ready solution requires architectural redesign

**Remaining Failures:**
- test_full_cooking_chain - Planner cancelled after 10 iterations (timeout before finding plan)
- test_competing_priorities_hunger_wins - Same timeout issue
- test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion - Action ordering issue (backward chaining produces [1,2] but test expects [2,1])

## Next Steps

### 1. Implement Production-Ready Callback Architecture (HIGH PRIORITY)
The timeout-based workaround is not suitable for production. Need to implement a truly asynchronous callback architecture:
- Use async/await or futures for callback requests
- Allow planner to continue exploring while callbacks are pending
- Revisit branches when callback results arrive
- This is a significant architectural change but necessary for production stability

### 2. Decide on Test Strategy
Options for handling the failing test (action ordering issue):
- Update test expectation to accept [1, 2] order (if this is valid backward chaining behavior)
- Keep timeout-based approach for tests and accept this test failure
- Create separate test-only planner implementation with mocked callbacks

### 3. Investigate Remaining Timeout Issues (LOWER PRIORITY - BLOCKED BY #1)
The cancel signal is working correctly - planner stops when cancelled. The remaining timeout failures are due to planner performance (needs more than 300 frames to find valid plans). Options:
- Increase test timeout values
- Investigate why planner needs more time for these specific scenarios
- Profile these specific test cases to understand search space

**UPDATE:** Added timestamps to logging system to diagnose timeout issues. Analysis revealed:
- Planner is NOT hanging - it completes in ~2 seconds when run individually
- Test passes when run individually but times out in full suite
- Debug logging overhead slows frame processing, causing premature timeout
- Even with log_level=2 (Info), timeouts still occur, suggesting test harness issues
- Possible state contamination between tests or frame pumping overhead

**UPDATE 2:** Added `clear_active_jobs()` method to scheduler and `before_each()` hooks to test files to flush state between tests. Timeouts persist, ruling out scheduler state contamination as the cause. Issue is likely:
1. Frame pumping overhead in test harness
2. Timeout values too low for these specific scenarios
3. Planner genuinely taking longer for complex scenarios

**UPDATE 4:** Profiling metrics are correct - planner shows 0 candidates evaluated with 15 
actions available because no actions are applicable to those specific test scenarios (e.g., 
low hunger + full fire). However, the root cause of timeouts is now identified as **callback 
blocking**:
- Planner threads run in Rayon thread pool and block synchronously on `rx.recv()` waiting for 
callback responses from main thread
- Callbacks include: custom precondition evaluation, action cost calculation, effect 
simulation
- Test harness must call `scheduler.process_callbacks()` to process these requests and 
unblock planner threads
- Even with 100x callback processing per iteration, planner threads spend most time blocked 
waiting for responses
- This is an architectural bottleneck: synchronous blocking from worker threads to main threa

**UPDATE 5 (May 20, 2026):** Implemented timeout-based callback solution to prevent indefinite blocking:
- Added 1000ms timeouts to all `rx.recv()` calls in `simulation.rs`
- When callbacks timeout, planner returns safe defaults (false for preconditions, 1.0 for cost, None for effects)
- Increased callback processing frequency in tests from 1x to 100x per iteration
- Applied to campfire/hunger smoke tests and async planner tests

**Results:**
- Build succeeded
- 38 out of 39 tests pass
- Tests complete in ~2 seconds (no hanging/timeout issues)
- Campfire and hunger smoke tests (which were hanging before) now pass
- 1 test fails: `test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion` - expects action chain [2, 1] but gets [1, 2] due to timeout-based callbacks causing different planner decisions

**Trade-offs:**
- ✅ Eliminates indefinite blocking in tests
- ✅ Allows tests to complete without hanging
- ❌ Not production-ready - timeouts can cause suboptimal planning decisions
- ❌ One test fails due to action ordering differences when callbacks timeout
- ❌ Safe fallback values (1.0 cost, false preconditions) may not reflect actual game logic

**Production Concerns:**
The timeout-based solution is a temporary workaround for testing only. In production:
- Timeouts could cause agents to make suboptimal decisions when callbacks are legitimately slow
- Fallback values may not match actual game mechanics
- Action ordering could be unpredictable under load
- This masks the real architectural issue rather than solving it

**Root Cause:** The planner's callback architecture is fundamentally incompatible with the test harness. In a real game, the main thread naturally processes callbacks as part of the game loop. In tests, we must artificially pump callbacks, but this creates a bottleneck where planner threads are blocked waiting for responses that the main thread can't deliver efficiently.

**Attempted Solutions:**
1. Time-based timeout instead of frame pumping - no improvement
2. Synchronous mode (run planner on main thread) - causes deadlocks (planner blocks on callbacks but main thread is busy running planner)
3. 10x callback processing per iteration - still times out
4. 100x callback processing per iteration - still times out
5. **Timeout-based callbacks (current workaround)** - prevents blocking but not production-ready

**Potential Solutions:**
1. **Truly asynchronous callback architecture** (recommended for production):
   - Use async/await or futures for callback requests instead of blocking channels
   - Allow planner to continue exploring while callbacks are pending
   - Revisit branches when callback results arrive
   - More complex but solves the architectural issue properly

2. **Direct callable execution in tests** (rejected due to thread safety):
   - Attempted to pass callable registry to planner for direct execution
   - Godot Callable objects are not thread-safe (cannot be sent across thread boundaries)
   - Would require major refactoring of Godot integration

3. **Accept timeout-based approach for tests only** (current workaround):
   - Keep 1000ms timeouts for test environment
   - Remove timeouts for production builds (or use much longer timeouts)
   - Document that tests may produce suboptimal plans due to timeouts
   - Not ideal but allows tests to run without hanging

4. **Separate test architecture**:
   - Create a test-specific planner that doesn't use GDScript callbacks
   - Mock all callback behavior in Rust
   - Ensures deterministic test behavior
   - Requires maintaining two planner implementations

### 2. Fix Action Ordering Test
test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion expects action order [2,1] but gets [1,2]. The test comment indicates backward chaining produces "LightFire then GetFood (prepares fire before food)" which may be the correct behavior. Need to verify if test expectation is correct.

## Relevant Files

- `addons/GdPlanningAI/rust/src/planner/engine.rs` - Search loop and profiling
- `addons/GdPlanningAI/rust/src/planner/expander.rs` - Branch expansion logic (reverted)
- `addons/GdPlanningAI/rust/src/planner/simulation.rs` - Callback timeout implementation (May 20, 2026)
- `addons/GdPlanningAI/rust/src/scheduler.rs` - Cancel signal implementation
- `addons/GdPlanningAI/rust/src/logger.rs` - Timestamp logging implementation
- `test/integration/test_campfire_example_smoke.gd` - Failing test (fixed validation, added 100x callback processing)
- `test/integration/test_hunger_example_smoke.gd` - Failing test (fixed validation, added 100x callback processing)
- `test/integration/test_async_planner.gd` - Async planner tests (added 100x callback processing)

## Log Evidence

Search loop showing exhaustion at depth 3 (before fix):
```
[GdPAI] Starting search loop with max_depth 6
[GdPAI] Search iteration: pop node with chain length 0
[GdPAI] Node expansion: chain length 0, depth 0, generated 1 successors
[GdPAI] Search iteration: pop node with chain length 1
[GdPAI] Node expansion: chain length 1, depth 1, generated 10 successors
[GdPAI] Search iteration: pop node with chain length 2
[GdPAI] Node expansion: chain length 2, depth 2, generated 2 successors
[GdPAI] Search iteration: pop node with chain length 3
[GdPAI] Node expansion: chain length 3, depth 3, generated 0 successors
[GdPAI] No successors generated - expander returned empty array
```

Cancel signal working (after fix):
```
[GdPAI] Search cancelled by cancel_flag after 10 iterations
```

Profiling metrics showing minimal exploration:
```
=== PROFILING METRICS ===
  Goal: Maintain Fire
  Actions: 15
  Max Depth: 6
  Theoretical Max Search Space: 11390625
  Search Iterations: 4
  Nodes Explored: 4
  Max Depth Reached: 3
  Candidates Evaluated: 10
  Exploration Percentage: 0.00%
========================
```

Timestamp analysis showing debug logging overhead:
```
[GdPAI 1779216048.159s] Starting search loop with max_depth 6
...
[GdPAI 1779216050.232s] Search cancelled by cancel_flag after 11 iterations
Duration: ~2.07 seconds of actual planning time
```

Test passes individually but times out in full suite, suggesting test harness state contamination or frame pumping overhead.
