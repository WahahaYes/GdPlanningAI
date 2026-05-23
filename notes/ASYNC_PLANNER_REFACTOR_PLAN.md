# Async Planner Refactor Plan

This plan addresses the performance bottlenecks and architectural failures identified in the current blocking callback implementation.

## 1. Problem Statement

1.  **Sequential Frame Blocking**: Each action in a "Ripple" (forward validation) takes one Godot frame to evaluate due to blocking `rx.recv()`. A plan with 5 actions takes 5 frames to validate a *single* branch.
2.  **Thread Pool Starvation**: Rayon threads are blocked during callbacks, preventing them from exploring other branches or handling other jobs.
3.  **Redundant Callbacks**: The planner re-simulates the beginning of action chains thousands of times, flooding the Godot main thread with redundant requests.
4.  **Routing Mismatch**: Previous attempts at non-blocking failed because they mixed receiver-based and channel-based routing without unique request IDs.

## 2. Phase 1: Non-Blocking "Parked" Search

Instead of blocking worker threads, the planner will yield control back to the scheduler when a callback is required.

### 2.1. Request ID Routing
-   Introduce a `request_id: usize` to `CallbackRequest`.
-   The `GdPAIPlanScheduler` will include this `request_id` in the `PlannerCallback` it sends back to the engine.
-   This eliminates the "Routing Mismatch" by allowing the engine to match responses to specific parked branches.

### 2.2. Suspendable Planner Engine
-   Refactor `PlannerEngine` to store its state (A* heap, visited set, current goal) internally.
-   **Resumption-First Policy**: The engine will always prioritize processing incoming callback results before popping new nodes from the search controller. This ensures that deep, promising branches are resumed as soon as they are unblocked.
-   **Parked Node Storage**: Add a `parked_nodes: HashMap<RequestID, SearchNode>` to the engine to hold nodes waiting for Godot's response.
-   Modify `plan()` to return a `PlannerRunResult`:
    -   `Success(PlanResult)`
    -   `Failure`
    -   `Pending(RequestID)`
-   When a `Pending` result is returned, the engine yields the Rayon thread.

### 2.4. Generalization Across Algorithms
By re-injecting unblocked branches into the `SearchController`, we maintain the integrity of each algorithm:
-   **A* (Heap)**: The unblocked branch's successors are pushed into the heap with their $f = g + h$ score. If they are still the "cheapest" path, they will be the next nodes popped.
-   **DFS (Stack)**: The unblocked branch's successors are pushed onto the stack. Since DFS is LIFO, this resumes the "deepest" path immediately.
-   **Dijkstra (Heap)**: Similar to A*, the branch is prioritized by its actual cost $g$.

## 3. Phase 2: Simulation Memoization (Caching)

Reduce the $O(N^2)$ callback volume by caching simulation results.

### 3.1. State Hashing
-   Implement `Hash` for `BlackboardSnapshot` and `ProvisionSpec` vectors.
-   Ensure snapshots are "stable" (sorted properties) for consistent hashing.

### 3.2. Simulation Cache
-   Add a `simulation_cache: HashMap<(ActionIdx, StateHash, ProvisionsHash), (NewState, Cost)>` to the `SearchContext`.
-   Before sending a callback, the simulation layer checks the cache.
-   **Result**: Redundant "Ripples" will hit the cache for all but the newly added action, reducing callback volume by >90%.

## 4. Phase 3: Logic Fix for "Hallucinations"

Fix the bug where un-selected provisions are used in optimistic simulation.

### 4.1. Chain-Aware Optimism
-   Modify `find_candidates` to only use provisions that are either:
    1.  Provided by the `InitialState`.
    2.  Provided by an action already in the current `action_chain`.
-   This prevents the planner from "hallucinating" that a goal is satisfied before its prerequisite actions are even considered.

## 5. Success Criteria

1.  **Zero Thread Blocking**: Worker threads must never call `recv()` or `wait()`.
2.  **Increased Throughput**: The planner should be able to explore multiple branches per frame if some are waiting on callbacks.
3.  **Test Stability**: `test_async_planner.gd` must pass without hitting the 300-frame timeout for complex plans.
4.  **Reduced Callback Volume**: Debug logs should show simulation cache hits for repeated actions.

## 7. Current Status (May 23, 2026 - End of Session)

### 7.1. Last Known Working Commit: `b227b13`
At commit `b227b13`, the planner was overcoming timeout issues. However, significant changes have been staged/implemented since then to address architectural debt and logic edge cases.

### 7.2. Audit of Changes since `b227b13` (Potential Regression Sources)

1.  **Engine Batching**: 
    -   **Change**: Modified `step_search` to continue popping from the controller after parking a node.
    -   **Intent**: Fix "Sequential Frame Blocking" by sending multiple requests per frame.
    -   **Risk**: Might be sending too many requests at once, overwhelming the main thread or creating search depth faster than the harness can keep up.

2.  **Phase 3 (Hallucination Fix - Softened)**:
    -   **Change**: In `find_candidates`, removed application of existing branch requirements to hypothetical state. In `expand_branch`, applied requirements optimistically for *all* ungrounded steps.
    -   **Intent**: Prevent "hallucinated" successes while maintaining search direction.
    -   **Risk**: Increased search space or cache misses during discovery due to stricter starting states.

3.  **Direct Callback Routing**:
    -   **Change**: Removed bridge threads. `CallbackRequest` now carries `Sender<PlannerCallback>` directly.
    -   **Intent**: Reduce thread overhead and latency.
    -   **Risk**: Changes the precise timing of when nodes are unparked, potentially exposing race conditions in `PlannerEngine::step_search`.

4.  **Termination Logic Guard**:
    -   **Change**: Engine returns `Pending` if `parked_nodes` is not empty, even if the search controller is empty.
    -   **Intent**: Prevent premature "No Plan Found" results while waiting for final callbacks.
    -   **Risk**: Extends the runtime of failing searches, potentially contributing to timeouts.

5.  **Float Quantization Tuning**:
    -   **Change**: Increased rounding in `calculate_hash` from 3 decimal places to 2.
    -   **Intent**: Improve cache stability.
    -   **Risk**: Unlikely to cause timeouts directly, but could cause incorrect cache hits if the precision is too low.

### 7.3. Known Issues
-   **Timeouts in Godot Integration Tests**: Despite batching, complex scenarios (Campfire, Cooking) are timing out. 
-   **Stable Regression**: Integration tests now fail on action order/cost when DFS is used, but timeout when A* is used.

## 8. Next Steps & Follow Ups

### 8.1. Restore A* Efficiency
-   **Problem**: `BestCost` strategy with the new non-blocking logic is exploring too many branches per frame, overwhelming the Godot main thread or exceeding the frame budget.
-   **Action**: Investigate why `AStar` is generating so many unique callback requests compared to the previous blocking implementation.

### 8.2. Cache Jitter Investigation
-   Even with float rounding, we may be seeing cache misses due to subtle state differences in the `provisions_hash` or `agent_state_hash`.
-   **Action**: Add detailed logging to track cache HIT vs MISS ratios during complex plans.

### 8.3. Search Algorithm Tuning
-   Experiment with hybrid strategies (e.g., depth-limited search or more aggressive pruning) to ensure the planner returns a "good enough" plan within the 300-frame limit.

