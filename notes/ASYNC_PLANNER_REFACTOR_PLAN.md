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

## 7. Current Status (May 22, 2026)

### 7.1. Implementation State
-   **Phase 1 & 2** are partially complete. The engine is suspendable, uses a `SearchContext` with Arc-shared data, and implements `SimulationKey` parameter-based caching.
-   **Build Status**: `make build-release` is functional.

### 7.2. The "Success Key" Error
Integration tests are crashing with `Invalid access to property or key 'success'`. 
- **Cause**: This is a secondary error triggered by a **timeout** in the GDScript test harness (`_submit_plan_and_wait`). 
- When the timeout is hit, the harness fails the test but continues execution, returning an empty `Dictionary {}`. The calling test then attempts to access `result["success"]`, causing the crash.

### 7.3. Timeout Hypotheses (The "Ping-Pong" Bottleneck)

Despite the non-blocking refactor, we are still seeing timeouts. Log analysis reveals a **Multi-Step Parking** issue:

1.  **Sequential Action Re-Parking**: 
    - Many actions define both `cost_callable` and `effect_callable`. 
    - Current logic: Node parks for Cost -> Resumes -> Immediately parks for Effect. 
    - This still requires 2 full Godot frames per action evaluation.
2.  **Candidate Discovery Explosion**:
    - `find_candidates` evaluates ALL potential actions. If 10 actions require callbacks for cost/validity and aren't cached, the node might park-and-resume 10 times before even starting the Ripple validation.
3.  **Simulation Key Collision/Miss**:
    - If `agent_state_hash` or `provisions_hash` changes subtly between discovery and ripple (e.g., due to float noise or unsorted provisions), the cache hits will fail, forcing redundant blocking calls.

## 8. Next Steps: Correcting the Resumption Flow

To break the "yield-resume-yield" loop, we need to:
1.  **Batch Callback Processing**: Allow the engine to process multiple ready callbacks in a single "Step" before yielding again.
2.  **Speculative Discovery**: Modify `find_candidates` to skip actions requiring callbacks if we are already in a "Resuming" state, or prioritize actions that are already cached.
3.  **Tighten Float Stability**: Ensure `calculate_hash` is extremely aggressive about rounding float values to prevent hash jitter.
