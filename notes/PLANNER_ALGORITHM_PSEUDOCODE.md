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
- **Discovery Optimization (Optional)**: Actions can be simulated against the `InitialState` once and cached (`DiscoveryCache`) to avoid request floods during candidate search. This is a toggleable optimization.

---

## 2. Core Data Structures

### `PlanBranch`
- `action_chain`: `[P, A, B]` (Backward-chained actions).
- `action_bindings`: `Vec<(pos, BindingData)>` (Data passed to simulations, e.g., which object was picked up).
- `open_preconditions`: Physical needs not yet met by `InitialState`. Stored as `(consumer_index, Precondition)`.
- `open_requirements`: Symbolic needs not yet satisfied. Stored as `(consumer_index, Requirement)`.
- `state`: One of `Initializing`, `Searching`, `Rippling`, `Verifying`.
- `simulation_index`: Current progress index within the `action_chain` or `goal_preconditions`.
- `current_agent`: Agent Blackboard resulting from action at `simulation_index - 1`.
- `current_world`: World Blackboard resulting from action at `simulation_index - 1`.
- `cost`: Total cumulative cost.

### `SearchNode` (A* Wrapper)
- `branch`: `PlanBranch`.
- `resumed`: Boolean flag (true if just woke up from callback).
- `callback_response`: Data payload from Godot (Updated snapshots, Float, or Bool).

---

## 2. Core Search Loop (`step_search`)

This loop is executed repeatedly by the background thread. It yields if a Godot callback is needed or if it exceeds its `ITERATION_BUDGET`.

### 3.1. Phase 1: Resume Callbacks
1. Drain the `response_channel`.
2. For each `response`:
   - Find all `ParkedNodes` waiting for this `request_id`.
   - For each node:
     - `node.callback_response = response`
     - `node.resumed = true`
     - Push `node` back into the `PriorityQueue`.

### 3.2. Phase 2: A* Iteration
1. Pop `node` from `PriorityQueue`.
2. **Visited Check**: If `!node.resumed` AND `visited_set.contains(node.fingerprint)`, then **CONTINUE**.
   - *Note: Fingerprint = (open_needs, state).*
3. Insert into `visited_set`.
4. `node.resumed = false`.

### 3.3. Phase 3: State Machine Processing
Based on `node.branch.state`:

#### **State: `Initializing`** (Grounding Goal)
- `target_index = action_chain.length` (The Goal's position at the end of the chain).
- Evaluate `Goal.precondition[simulation_index]` against `InitialState`.
- **If Pending(id)**: `Park(node, id)`, **Yield Loop**.
- **If Ready(bool)**:
  - If `false`: Add `(target_index, precondition)` to `branch.open_preconditions`.
  - `simulation_index++`. 
  - If all goal preconds checked: `branch.state = Searching`.
  - Push `node` back to queue.

#### **State: `Searching`** (Expansion)
- **Check Complete**: If `open_preconditions` and `open_requirements` are empty:
  - `branch.state = Verifying`, `simulation_index = 0`.
  - Reset `current_agent/world` to `InitialState`.
  - Push `node` back to queue, **CONTINUE**.
- **Expand**: Find all `Action A` that satisfy **at least one** open need (using `find_candidates`).
  - For each candidate:
    - `NewBranch` = Prepend `A` to chain.
    - **Frontier Update**: 
      - Remove `open_preconditions` and `open_requirements` that are satisfied by `A`'s simulated effects/provisions. 
      - *Note: A precondition is satisfied if it is met at its specific point in the sequence; it does NOT need to remain true for the rest of the chain.*
      - Increment `consumer_index` of all remaining downstream needs by +1.
    - **Add New Needs**: Add `A.preconditions` and `A.requirements` at `consumer_index = 0`.
    - **Reset Ripple**: `NewBranch.state = Rippling`, `simulation_index = 0`.
    - Push `NewBranch` to queue.

#### **State: `Rippling` / `Verifying`** (Simulation & Grounding)
1. **Point-in-Time Check**: Before simulating `action_chain[simulation_index]`:
   - Identify all `open_preconditions` where `consumer_index == simulation_index`.
   - Evaluate them against `current_agent/world` (the state after all preceding actions).
   - **If Ready(true)**: The need is satisfied at this point in the chain.
   - **If Ready(false)**: The chain is broken at this step (Invalidate branch or keep as open need).
2. **Simulate**: Simulate `action_chain[simulation_index]` against `current_state`.
   - **If Pending(id)**: `Park(node, id)`, **Yield Loop**.
   - **If Ready(result)**:
     - Update `branch.current_agent`, `branch.current_world` and `branch.cost`.
     - `simulation_index++`.
     - If finished:
       - `Rippling` -> `branch.state = Searching`.
       - `Verifying` -> Perform final `GoalCheck`. If satisfied, **RETURN SUCCESSFUL PLAN**.
     - Push `node` back to queue.

---

## 4. Candidate Discovery (`find_candidates`)

This is where we identify which actions are relevant to the current `open_needs`.

1. **Validity Filter**: Check `Action.validity_checks` against `InitialState`. Skip if any fail (e.g., on cooldown).
2. **Discovery Preview** (Optional Optimization): 
   - *Note: If DiscoveryCache is disabled, skip to step 3 and use fresh simulations.*
   - Check `DiscoveryCache` for `Action A`.
   - If missing:
     - Check `DiscoveryPending` map.
     - If pending: **Skip A** for this iteration (Wait for Godot).
     - If not pending: Run the custom GDScript `simulate_effect` for `A` against `InitialState`.
       - If `Pending(id)`: Mark `DiscoveryPending[A] = id`, **Return Pending(id)**.
       - If `Ready`: Store result in `DiscoveryCache`.
3. **Hybrid Satisfaction Check**:
   - **Symbolic Layer**: Does `A`'s **Provisions** satisfy any `open_requirements`?
   - **Simulation Layer**: Does `A`'s **Discovery Result** (the simulated state) satisfy any `open_preconditions`?
   - *Note: This allows actions with unadvertised effects to be discovered if the simulation reveals they satisfy a physical need.*
4. **Qualification**:
   - `A` is a candidate if it satisfies **at least one** open requirement OR **at least one** open precondition.

---

## 5. Summary of Routing Logic

| If Simulation Result is... | Engine Action | Main Thread Action |
| :--- | :--- | :--- |
| **`Ready(value)`** | Continue current branch logic. | None. |
| **`Pending(id)`** | Store node in `ParkedNodes[id]`, Yield execution of this job. | Run the GDScript `Callable`, send result to `response_channel`. |
| **`Invalid`** | Discard the current node. | None. |
| **`Budget Exceeded`** | Push current node back to `PriorityQueue`, Yield execution. | None. |
