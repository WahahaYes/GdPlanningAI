# Async GOAP Planner: Definitive Implementation Pseudocode

## Preliminary: The Hybrid Planner Model
The GdPlanningAI planner is a **Hybrid Backward-Chaining Planner** that combines the efficiency of symbolic GOAP with the power of rich scene simulation.
- **Symbolic Layer**: Uses traditional Requirements and Provisions for fast causal link discovery and initial pruning.
- **Simulation Layer**: Leverages custom GDScript callables for `simulate_effect`, `eval_precondition`, and `calculate_cost`. This allows the planner to reason about complex scene information (spatial distance, object properties, navigation) that cannot be easily captured in simple bitmasks.
- **Async Execution**: The planning process is non-blocking, yielding to the Godot main thread whenever a GDScript simulation or check is required.

This document describes the non-blocking, async-first backward-chaining implementation.

## 1. Key Principles
- **Incremental Progress**: Every simulation step (cost, effect, precondition) can yield a `Pending` result, requiring a Godot callback.
- **State Machine Branches**: A plan branch is not just a list of actions; it is a state machine that tracks its own simulation progress.
- **Needs-Based Identity**: A branch's search identity is defined by its **Open Needs** (unmet preconditions and requirements).
- **Split Constraint Discovery**: Requirements are strictly sequential (must satisfy `pos 0`), while Preconditions are unrestricted (can satisfy any point in the chain).
- **Greedy Clearing**: Satisfying one requirement (e.g. `at_target`) clears all identical requirements in the chain to prevent congestion.
- **Async Concurrency**: `find_candidates` returns "Ready" actions immediately to keep search threads busy while other simulations are "Pending".

---

## 2. Core Data Structures

### `PlanBranch`
- `action_chain`: `[P, A, B]` (Backward-chained actions).
- `action_costs`: `[1.0, 2.5, 0.5]` (Grounded cost for each action).
- `action_bindings`: `Vec<(pos, BindingData)>` (Data passed to simulations).
- `open_preconditions`: Stored as `(pos, PreconditionSpec)`. 
- `open_requirements`: Stored as `(pos, RequirementSpec)`.
- `state`: One of `Initializing`, `Searching`, `Rippling`, `Verifying`.
- `simulation_index`: Current progress index during forward phases.
- `cost`: Total cumulative grounded cost (recalculated via `sum()`).
- `fingerprint`: `(goal_index, Vec<PreconditionSpec>, Vec<RequirementSpec>, state)` (Positions are stripped to catch causal loops).

### `SearchNode` (A* Wrapper)
- `branch`: `PlanBranch`.
- `resumed`: Boolean flag (true if just woke up from callback).
- `callback_response`: Data payload from Godot (Updated snapshots, Float, or Bool).

---

## 2. Core Search Loop (`step_search`)

This loop is executed repeatedly by the background thread. It yields if a Godot callback is needed or if it exceeds its `ITERATION_BUDGET`.

### 3.1. Phase 1: Resume Callbacks
1. Drain `response_channel`.
2. For each `response`:
   - Update Discovery Cache with results (cost/effect/precond).
   - Find all `ParkedNodes` waiting for this `request_id`.
   - **Check Cancellation**: If `cancel_flag` is set, clear parked nodes and abort.
   - For each node:
     - `node.callback_response = response`, `node.resumed = true`.
     - Push `node` back into `PriorityQueue`.

### 3.2. Phase 2: A* Iteration
1. Pop `node` from `PriorityQueue`.
2. **Visited Check**: If `visited_set.contains(node.fingerprint)` and `cost >= visited_cost`, then **PRUNE**.
3. **Optimality Check**: If `node.cost >= best_cost`, then **PRUNE**.
4. `node.resumed = false`.

### 3.3. Phase 3: State Machine Processing
Based on `node.branch.state`:

#### **State: `Searching`** (Expansion)
1. **Check Complete**: If `open_preconditions` and `open_requirements` are empty:
   - `branch.state = Verifying`, `simulation_index = 0`.
   - Reset `current_agent/world` to `InitialState`.
   - Push back to queue.
2. **Expand**: Call `find_candidates(branch, response)`.
3. **Process Ready Candidates**: For each `Candidate C`:
   - `NewBranch = branch.clone()`, Prepend `C.action` at `pos 0`.
   - **Greedy Clearing**: 
     - Remove the matched requirement.
     - Scan entire `open_requirements` and remove ALL identical specs.
     - Remove all preconditions satisfied by `C`'s discovery result.
   - **Frontier Update**: Offset `pos` of all remaining needs by +1.
   - **Add New Needs**: Add `C.action.requirements` and `C.action.preconditions` at `pos 0`.
   - **Deduplicate**: Check `HashSet` to ensure we don't add redundant needs already in the chain.
   - **Stay in Searching**: Keep `state = Searching` to allow further backward chaining.
4. **Handle Pending**: If `find_candidates` returned a `pending_id`:
   - `Park(node, id)`. (Note: Ready candidates were already queued, so search continues).

#### **State: `Rippling` / `Verifying`** (Forward Simulation)
1. **Validation Entry**: Only enter these states once all symbolic needs are resolved.
2. **Check Current State**: Evaluate `open_preconditions` where `pos == simulation_index` against `current_agent/world`.
   - If `false`, return `Invalid`. If `true`, remove precondition.
3. **Simulate Step**: Run `simulate_action(action_chain[simulation_index])`.
   - Update snapshots and `action_costs`.
   - **Accumulate Provisions**: Check if the action's provisions satisfy any downstream `open_requirements`.
   - `simulation_index++`.
4. **Terminal Check**: If `simulation_index == chain.len()`:
   - If `Verifying`: If all needs empty, **Record Best Plan**.

---

## 4. Candidate Discovery (`find_candidates`)

This is where we identify which actions are relevant to the current `open_needs`.

1. **Partial Delivery**: Loop through all actions. 
2. **For each Action**:
   - Check `validity_checks` against `InitialState`. Skip if any fail (e.g., on cooldown).
   - **Match Requirements (Strict pos 0)**:
     - Only match if `pos == 0`.
     - Ensures movement actions (`Go To`) are inserted exactly where needed.
   - **Match Preconditions (Unrestricted)**:
     - Match any `pos`. Preserves optimality for persistent state properties.
   - **Simulate Effect**: Use Discovery Cache or request callback.
3. **Return Result**: `CandidatesResult { ready_list, pending_id }`.

---

## 5. Summary of Routing Logic

| If Simulation Result is... | Engine Action | Main Thread Action |
| :--- | :--- | :--- |
| **`Ready(value)`** | Continue current branch logic. | None. |
| **`Pending(id)`** | Store node in `ParkedNodes[id]`, Yield execution of this job. | Run the GDScript `Callable`, send result to `response_channel`. |
| **`Invalid`** | Discard the current node. | None. |
| **`Budget Exceeded`** | Push current node back to `PriorityQueue`, Yield execution. | None. |
