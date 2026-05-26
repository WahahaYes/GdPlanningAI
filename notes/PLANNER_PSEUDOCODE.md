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
- `action_bindings`: `Vec<(pos, BindingData)>` (Data passed to simulations. Associated with both provider and consumer).
- `open_preconditions`: Physical needs not yet met by `InitialState`. Stored as `(pos, Precondition)`. `pos` is the absolute index in `action_chain` where the condition must be true. Goal preconditions are anchored at `pos = chain.len()`.
- `open_requirements`: Symbolic needs not yet satisfied. Stored as `(pos, Requirement)`.
- `state`: One of `Initializing`, `Searching`, `Rippling`, `Verifying`.
- `simulation_index`: Current progress index within the `action_chain` or `goal_preconditions`.
- `current_agent`: Agent Blackboard resulting from action at `simulation_index - 1`.
- `current_world`: World Blackboard resulting from action at `simulation_index - 1`.
- `cost`: Total cumulative grounded cost (used for A* priority).
- `tree_node_id`: ID of the node in the debug search tree.

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
- Evaluate `Goal.precondition` against `InitialState`.
- **If Pending(id)**: `Park(node, id)`, **Yield Loop**.
- **If Ready(bool)**:
  - If `false`: Add `(0, precondition)` to `branch.open_preconditions`. (Anchored at the end of the empty chain).
  - `branch.state = Searching`.
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
      - Increment `pos` of all remaining downstream needs by +1.
    - **Binding Propagation**:
      - When `A`'s provision satisfies an `open_requirement` of action `B`:
        - Record the binding data for **position 0** (the provider `A`).
        - Record the binding data for the **consumer's position** (action `B`, offset by +1).
      - *Note: For `FactWildcard` provisions (like 'at_target'), the arguments are pulled from the satisfying `Requirement` and used to ground the discovery simulation.*
    - **Add New Needs**: Add `A.preconditions` and `A.requirements` at `pos = 0`.
    - **Reset Ripple**: `NewBranch.state = Rippling`, `simulation_index = 0`.
    - Push `NewBranch` to queue.

#### **State: `Rippling` / `Verifying`** (Simulation & Grounding)
0. **Ground Requirements**:
   - If `simulation_index == 0`, check `open_requirements` at `pos=0` against `InitialState` provisions.
   - After each simulated action `i`, check `open_requirements` where `pos > i` against the action's provisions.
   - Remove any that are satisfied.
1. **Point-in-Time Check**: Before simulating `action_chain[simulation_index]`:
   - Identify all `open_preconditions` where `pos == simulation_index`.
   - Evaluate them against `current_agent/world` (the state after all preceding actions).
   - **If Ready(true)**: The need is satisfied at this point in the chain. Remove it.
   - **If Ready(false)**: The chain is broken at this step. Return `Invalid`.
2. **Simulate**: Simulate `action_chain[simulation_index]` against `current_state`.
   - **If Pending(id)**: `Park(node, id)`, **Yield Loop**.
   - **If Ready(result)**:
     - Update `branch.current_agent`, `branch.current_world` and `branch.cost`.
     - `simulation_index++`.
     - If finished (`simulation_index == chain.length`):
       - **If `Rippling`**: 
         - Check if any `open_preconditions` anchored at `chain.length` (Goal Preconditions) are now satisfied by the final state.
         - `branch.state = Searching`.
       - **If `Verifying`**:
         - Final Goal Check: Ensure no `open_preconditions` or `open_requirements` remain anywhere in the chain.
         - If satisfied, **RETURN SUCCESSFUL PLAN**.
     - Push `node` back to queue.

---

## 4. Candidate Discovery (`find_candidates`)

This is where we identify which actions are relevant to the current `open_needs`.

1. **Validity Filter**: Check `Action.validity_checks` against `InitialState`. Skip if any fail (e.g., on cooldown).
2. **Discovery Preview** (Binding-Aware): 
   - *Note: Discovery is now keyed by `(Action, Bindings)`.*
   - If an action has a **Wildcard Provision** (e.g. `at_target`), it is evaluated separately for **each** requirement it could satisfy.
   - Run the custom GDScript `simulate_effect` and `calculate_cost` for `A` against `InitialState` **using the bindings from the requirement**.
   - Store results in `DiscoveryCache[(A, Bindings)]`.
3. **Hybrid Satisfaction Check**:
   - **Symbolic Layer**: Does `A`'s **Provisions** satisfy any `open_requirements`?
     - *Strict Enrollment*: Requirements like `BindingInSet` are verified against `InitialState` object groups immediately. If a provision refers to an object not in the required group, the match fails.
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
