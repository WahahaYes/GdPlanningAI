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

**Remaining Failures:**
- test_full_cooking_chain - Planner cancelled after 10 iterations (timeout before finding plan)
- test_competing_priorities_hunger_wins - Same timeout issue
- test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion - Action ordering issue (backward chaining produces [1,2] but test expects [2,1])

## Next Steps

### 1. Investigate Remaining Timeout Issues
The cancel signal is working correctly - planner stops when cancelled. The remaining timeout failures are due to planner performance (needs more than 300 frames to find valid plans). Options:
- Increase test timeout values
- Investigate why planner needs more time for these specific scenarios
- Profile these specific test cases to understand search space

### 2. Fix Action Ordering Test
test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion expects action order [2,1] but gets [1,2]. The test comment indicates backward chaining produces "LightFire then GetFood (prepares fire before food)" which may be the correct behavior. Need to verify if test expectation is correct.

## Relevant Files

- `addons/GdPlanningAI/rust/src/planner/engine.rs` - Search loop and profiling
- `addons/GdPlanningAI/rust/src/planner/expander.rs` - Branch expansion logic (reverted)
- `addons/GdPlanningAI/rust/src/scheduler.rs` - Cancel signal implementation
- `test/integration/test_campfire_example_smoke.gd` - Failing test (fixed validation)
- `test/integration/test_hunger_example_smoke.gd` - Failing test (fixed validation)

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
