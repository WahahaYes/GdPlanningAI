# Planner Design Revision: Optimistic Backward Search with Targeted Re-simulation

## The Core Conflict
The current planner performs **Backward Search** but requires **Forward State Knowledge** to evaluate arbitrary GDScript callables (preconditions, costs, and effects). Every time a predecessor action is inserted at the front of the chain, we need to know the state *after* that action to evaluate the next actions in the suffix.

## The Refined Design: Snapshot Sequences & Triggered Re-simulation

We will maintain the hybrid approach but optimize the simulation lifecycle to avoid $O(N^2)$ overhead and state corruption.

### 1. Snapshot Sequence (Internal State)
Instead of a single `accumulated_agent` snapshot, each `PlanBranch` stores a `Vec<BlackboardSnapshot>`:
*   `snapshots[0]` is the initial state (before the first action).
*   `snapshots[i+1]` is the state after `action_chain[i]`.
*   **Expansion**: When searching for candidates for a branch, we evaluate them against `snapshots[0]` (the state at the point of insertion).

### 2. The "Requirement-Triggered" Re-simulation
We do **not** re-simulate the entire suffix chain on every expansion. 
*   **Insertion**: When a predecessor is added, we simulate its effect once to create `snapshots[1]`.
*   **Suffix Propagation**: The existing suffix snapshots are shifted.
*   **Re-simulation Trigger**: We only re-call `simulate_effect` for an action in the suffix if:
    1.  One of its `RequirementSpec` dependencies was just satisfied by the new predecessor (e.g., `held_item` is now bound).
    2.  The predecessor modified a property that the suffix action reads (detected via symbolic diff or conservatively).

### 3. Benefits
1.  **Correct Grounding**: Predecessor actions are evaluated against the initial state, where they actually execute.
2.  **Performance**: Most expansions only require 1-2 `simulate_effect` calls rather than $N$.
3.  **Stability**: We avoid "optimistic placeholders" where possible, using real bound values once a provider is found.

## Next Steps for Continuation

### 1. Finalize Snapshot Sequence Implementation
*   **Location**: `BranchExpander::expand` in `expander.rs`.
*   **Logic**:
    1.  When a predecessor `P` is chosen, clone the parent `snapshots` vector.
    2.  Insert `P.simulate_effect(initial_state)` as the new `snapshots[1]`.
    3.  Shift all existing snapshots from the parent by 1 (e.g., `parent_snapshots[1]` becomes `child_snapshots[2]`).
    4.  **Targeted Re-simulation**: Iterate through the suffix chain starting from the action that was just "pushed" by the insertion. Only call `simulate_effect` again if the state from its new predecessor is significantly different or a requirement was resolved.
    5.  Update `new_branch.estimated_cost` using the ground-truth costs from these simulations.

### 2. Ground Predecessor Finding in Initial State
*   **Location**: `find_candidate_actions` and `action_candidates_for_needs`.
*   **Refactor**: These functions currently take the whole `branch`. They should specifically look at `branch.agent_snapshots[0]` (the `initial_state`) to evaluate if a candidate can be prepended.
*   **Optimism**: `EatHeldFoodAction` must still be found even if `held_item` is empty in `snapshots[0]`. We achieve this by passing `strict=false` to the hypothetical simulation during candidate discovery.

### 3. Strengthen Symbolic Pruning
*   **Issue**: We solved the "Eat Wood" problem by adding group requirements.
*   **Next Step**: Ensure that if an action's symbolic requirement is satisfied by a provision that contradicts its cost/effect logic (e.g., a "banana" that somehow doesn't restore hunger), the forward validation pass handles it, but the symbolic layer catches the most obvious mismatches.

### 4. Verification Suite
*   **Campfire Smoke Test**: Verify that the agent can still cook a potato (GoTo -> Dig -> GoTo -> Cook -> Eat). This requires 5 actions and will stress-test the snapshot shifting logic.
*   **A* Benchmark**: Compare branches expanded between DFS and A*. With state deduplication and cost normalization, A* should be significantly more efficient.

## Current State Summary
*   **AStarController**: Fully implemented with `BinaryHeap`.
*   **State Deduplication**: `NodeFingerprint` includes `StableSnapshot` (rounded floats) + `OpenNeeds`. This is critical because two paths to the same state are only identical if they have the same remaining requirements.
*   **Cost Scaling**: `GoToAction` costs are normalized (`/ 100.0`). Falling back to `1.0` if no target is bound.
*   **Callable Fixes**: `scheduler.rs` now handles `i64` to `f64` conversion and correctly resolves registry index 0.
*   **PlanBranch**: Struct fields `agent_snapshots` and `world_snapshots` are present but not yet correctly shifted/updated in the expander loop.
