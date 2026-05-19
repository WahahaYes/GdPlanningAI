# Planner Timeout and Exhaustion Investigation

## Problem Description

Integration tests (particularly campfire and hunger examples) are timing out with "Out of bounds get index '0'" errors. The planner appears to exhaust the search space without finding valid plans.

## Observed Issues

### 1. Test Timeout Not Sending Cancel Signal
- Test timeout (300 frames) occurs but planner continues running in background
- Cancel flag is not being set when test times out
- Planner continues processing after test failure
- Evidence: "Search iteration" messages continue appearing after test timeout error

### 2. Test Not Asserting Plan is Not None
- Test tries to access `plan[0]` without checking if plan is empty
- "Out of bounds get index '0'" error occurs when planner returns None/empty plan
- Test should assert plan is not None before accessing elements
- Location: `test/integration/test_campfire_example_smoke.gd:112`

### 3. Search Exhaustion at Depth 3
- Search loop iterates correctly but exhausts at chain length 3, depth 3
- Expander returns 0 successors at depth 3
- Search keeps popping nodes at depth 3 but cannot expand them
- Planner exhausts search space without finding complete plan
- Evidence: "Node expansion: chain length 3, depth 3, generated 0 successors"

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

The timeout is NOT caused by:
- Excessive search space exploration (only 0.00%-18.75% explored)
- Infinite loop in search (loop exits correctly)
- Controller stuck (nodes are being popped and processed)

The timeout IS caused by:
- Test timeout (300 frames) not canceling planner
- Planner exhausting search space without finding valid plan
- Test not handling None/empty plan result gracefully

## Next Debug Steps

### 1. Fix Test Timeout Cancel Signal
- Investigate how cancel_flag should be set on test timeout
- Ensure scheduler properly cancels in-flight planning jobs
- Verify cancel signal propagates to planner threads

### 2. Add Plan Validation in Tests
- Assert plan is not None before accessing elements
- Add graceful handling of empty plan results
- Test should fail with clear message if no plan found

### 3. Investigate Depth 3 Exhaustion
- Add logging to expander to show why 0 successors at depth 3
- Check if actions are being filtered incorrectly
- Verify preconditions/requirements are being evaluated correctly
- Compare with old DFS insertion order behavior

### 4. Check Branch Recovery Logic
- Verify controller correctly backtracks to depth 2 after depth 3 exhaustion
- Ensure alternative branches at depth 2 are explored
- Add logging to show branch selection and backtracking

## Relevant Files

- `addons/GdPlanningAI/rust/src/planner/engine.rs` - Search loop and profiling
- `addons/GdPlanningAI/rust/src/planner/expander.rs` - Branch expansion logic
- `addons/GdPlanningAI/rust/src/planner/controller.rs` - Search controller
- `test/integration/test_campfire_example_smoke.gd` - Failing test
- `addons/GdPlanningAI/plugin.cfg` - Log level configuration

## Log Evidence

Search loop showing exhaustion at depth 3:
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
