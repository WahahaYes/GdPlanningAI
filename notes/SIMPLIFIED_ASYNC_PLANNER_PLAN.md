# Simplified Async Planner Redesign

This plan aims to strip out the complexity of hashing and memoization to focus on a correct, debuggable, and non-blocking async planner implementation.

## 1. Problem Statement
The current implementation is bogged down by:
- **Hashing Overhead**: Stable snapshots and key hashing for every simulation step.
- **Memoization Complexity**: Global caches and pending request tracking that are difficult to debug and potentially hide race conditions.
- **Ripple Inefficiency**: Re-simulating the entire chain from scratch on every expansion, relying on caches to make it fast.

## 2. Core Concepts of the Redesign

### 2.1. Conditional Rippling (Smart Prefixing)
Instead of re-simulating the whole chain `[P, A, B, C]` every time we prepend `P`, we distinguish between two types of satisfaction:

1.  **Requirement Grounding (The Ripple)**: If `P` provides a provision (e.g., `held_item: "wood"`) that was a symbolic requirement of a later action (e.g., `Add Fuel`), we **must** perform a full forward ripple. This is because the later action's cost and effects might change now that it has a concrete object to work with.
2.  **Precondition Satisfaction (The Prefix)**: If `P` only satisfies a physical precondition (e.g., `at_location`) and doesn't ground any new symbolic requirements, we can **Simply Prefix** `P`:
    - `NewBranch.cost = P.cost + ParentBranch.cost`
    - `NewBranch.head_state = result of P`
    - `NewBranch.tail_state = ParentBranch.tail_state` (Assumed still valid for goal check)
    - **Deferred Verification**: When a branch has no open needs, we perform one final "Verification Ripple" to ensure the side effects of prefixes didn't invalidate the goal or significantly change the cost.

### 2.2. Suspension-Aware Branch State
To avoid re-simulating from scratch and hitting caches:
- A `PlanBranch` will track its `simulation_index`.
- If a simulation step `i` requires a Godot callback, the branch is parked.
- When resumed, it starts exactly at `simulation_index = i`.
- No global hashing needed; the branch carries its own partial simulation state.

### 2.3. Needs-Based Fingerprint (Simplified Visited Set)
To prevent infinite loops and redundant search without complex blackboard hashing:
- A `SearchNode` will be fingerprinted by its **Open Needs** (`open_preconditions` + `open_requirements`).
- If we reach the same set of needs via a different path, we can compare costs and prune.
- This eliminates the need for `StableSnapshot` and property-level hashing.

## 3. Implementation Steps

### 3.1. Simplify `PlanBranch`
- Remove `final_state_agent/world` (replace with `current_sim_state`).
- Add `simulation_index: usize`.
- Add `simulation_state: Option<(BlackboardSnapshot, BlackboardSnapshot)>`.

### 3.2. Simplify `simulation.rs`
- Remove `SimulationKey`.
- Remove `calculate_hash` and `calculate_provisions_hash`.
- `eval_precondition` and `simulate_action` no longer check a global cache.
- They return `Pending(request_id)` directly if a callback is needed.

### 3.3. Refactor `engine.rs` Search Loop
- The loop becomes a simple state machine:
  1. `Pop` node.
  2. If `node.branch` is not fully simulated (`sim_index < chain.len()`):
     - Run one simulation step.
     - If `Pending`: `Park` and `Yield`.
     - If `Ready`: Push back to controller.
  3. If `node.branch` is fully simulated:
     - Check `is_complete()`.
     - If complete: `Return Success`.
     - If not: `Expand` (find candidates).
     - Push new branches to controller.

### 3.4. Remove Memoization
- Delete `pending_requests` and `callback_results` from `SearchContext`.
- Delete `StableVariant`, `StableSnapshot`, and all hashing logic from `snapshot.rs`.

## 4. Benefits
- **Deterministic**: No cache collisions or hash jitter.
- **Traceable**: A branch's progress is clear in the debugger.
- **Resilient**: Resuming a branch doesn't depend on global state.
- **Responsive**: Iteration limits ensure Godot frames aren't blocked.

## 6. Current Status & Debugging (May 23, 2026)

### 6.1. Accomplishments
- **PlanBranch Simplified**: Replaced complex re-simulation ripples with a 4-state machine (`Initializing`, `Searching`, `Rippling`, `Verifying`).
- **Discovery Cache**: Added to `SearchContext` to prevent O(N*M) simulation explosions during candidate discovery. Each action is now simulated once per initial state.
- **Needs-Based Fingerprinting**: A* visited set now uses the branch's open needs instead of blackboard hashing.
- **Incremental Resumption**: Godot callback results are injected directly into resumed nodes.

### 6.2. Identified Issues
- **Profiling Bloat**: `PROFILING METRICS` are being dumped on every yield rather than just at the end of a goal search. This indicates the `step_search` loop is yielding but not terminating as expected.
- **Depth-0 Iteration Loop**: Logs show high iteration counts (e.g., 185) with `Max Depth: 0`. This suggests the engine is repeatedly processing the root node or failing to transition out of the `Initializing` state correctly.
- **Grounding Failure**: If the root node's preconditions (from the Goal) aren't being satisfied by the initial state, and no candidates are found to satisfy them, the search should terminate with failure rather than spinning.

### 6.3. Next Steps
1. **Fix Termination Logic**: Ensure profiling metrics only log on `Complete`.
2. **Debug Initializing State**: Verify that goal preconditions are correctly pushed to `open_preconditions` and that the state transitions to `Searching` exactly once.
3. **Trace Root Expansion**: Determine why candidates aren't being successfully prepended to increase search depth.
