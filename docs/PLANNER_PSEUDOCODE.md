# Backward-Chaining GOAP Planner Pseudocode

---

## 1. Overview

The planner is a **hybrid backward-chaining GOAP** search engine:
- **Symbolic layer:** Requirements and Provisions establish causal links.
- **Simulation layer:** GDScript callbacks evaluate costs, effects, and custom preconditions against agent/world snapshots.
- **Search:** Dijkstra (uniform-cost) via a modular `SearchHeuristic` trait. A* is available as a no-op placeholder until an admissible heuristic is designed.
- **Async:** Every Godot callback can yield `Pending`; the engine parks the node and resumes when the response arrives.

---

## 2. Core Data Structures

### `PlanBranch`
The mutable state of one partial plan under construction.
```
action_chain:         Vec<usize>              // Indices into SearchContext.actions
action_costs:         Vec<f64>                // Grounded cost per action
action_bindings:      Vec<(pos, name, values)>// Bindings for actions at specific positions
open_preconditions:   Vec<(pos, PreconditionSpec)>  // Needs to be satisfied by predecessor actions
open_requirements:    Vec<(pos, RequirementSpec)>   // Symbolic needs (e.g. at_target)
state:                BranchState             // Searching | Verifying
simulation_index:     usize                   // Current position during forward simulation
current_agent:        BlackboardSnapshot      // State after simulating simulation_index actions
current_world:        BlackboardSnapshot
cost:                 f64                     // Sum of action_costs (recalculated after each change)
tree_node_id:         usize                   // Debug tree node reference
```

### `SearchNode`
A wrapper that couples a branch with its async state.
```
branch:               PlanBranch
resumed:              bool                    // True if this node just received a callback response
callback_response:      Option<CallbackResponse>// Most recent Godot callback payload
expanded_candidates:  Vec<(action_idx, bindings)> // Candidates already processed; prevents dup children
```

### `PriorityNode`
Couples a `SearchNode` with its heap priority. The `BinaryHeap` is a max-heap, so `Ord` inverts the comparison for min-heap behaviour.
```
priority: f64   // Computed by SearchHeuristic.compute_priority()
node:     SearchNode
```

### `PlannerEngine`
```
ctx:               Arc<SearchContext>
max_depth:         usize
heuristic:         Box<dyn SearchHeuristic>
termination:       TerminationStrategy   // FirstComplete | BestCost
iteration_budget:  usize
queue:             BinaryHeap<PriorityNode>
parked_nodes:      HashMap<request_id, Vec<SearchNode>>
visited:           HashMap<SearchFingerprint, cost>
best_plan:         Option<PlanResult>
best_cost:         f64
```

---

## 3. Search Loop (`step_search`)

### Phase 1: Resume Callbacks
Drain all responses from `response_rx`.
For each `PlannerCallback`:
1. If it is a **Discovery response** (cost/effect/precondition cache):
   - Write the result into the appropriate discovery cache.
   - Remove the request from `discovery_pending` and `discovery_request_map`.
2. Find all **parked nodes** waiting on this `request_id`.
3. If `cancel_flag` is set, clear parked nodes and abort.
4. For each parked node:
   - Set `node.resumed = true`, `node.callback_response = response`.
   - Re-enqueue the node.

### Phase 2: Main Loop
Pop nodes from `queue` until empty or budget exhausted.

For each popped `PriorityNode`:
1. **Budget check:** If `iterations > iteration_budget`, re-enqueue node and return `Pending(0)`.
2. **Optimality prune (BestCost only):** If `heuristic.prune_threshold_met(node_priority, best_cost)`, re-enqueue and break.
3. **Cancellation check:** If `cancel_flag`, return `Complete(None)`.
4. **Visited prune:** If `!node.resumed` and `state == Searching`:
   - Compute `fingerprint()`.
   - If `visited` already has this fingerprint with `cost <= node.branch.cost`, skip (prune).
   - Otherwise, insert `(fingerprint, cost)` into `visited`.
5. **Clear expanded candidates:** If `node.resumed`, clear `expanded_candidates`.
6. **Cost prune:** If `node.branch.cost >= best_cost`, skip.

### Phase 3: State Machine

#### `BranchState::Searching` (backward expansion)

**Goal satisfied?** If `open_preconditions` and `open_requirements` are both empty:
- Transition to `Verifying`.
- Reset `simulation_index = 0`, `current_agent/world = initial_state`.
- Re-enqueue.

**Max depth?** If `action_chain.len() >= max_depth`, skip (prune).

**Expand:** Call `find_candidates(branch, ctx)`.

For each **ready** `Candidate`:
1. Skip if `(action_idx, bindings)` already in `expanded_candidates`.
2. `new_branch = branch.clone()`.
3. Prepend `candidate.action_idx` to `new_branch.action_chain`.
4. Insert discovery cost into `new_branch.action_costs[0]`.
5. Call `new_branch.shift_positions(1)` — increments all `pos` values in open needs and bindings.

**Update open needs:**
1. **Remove satisfied requirements:**
   - For each requirement matched by the candidate, do **greedy clearing**: scan ALL `open_requirements` and remove every entry with an identical spec.
   - Collect binding data from the provision that satisfied each requirement.
   - Add bindings for the provider (position 0) and for every cleared consumer (their positions).
2. **Remove satisfied preconditions:** Remove all preconditions listed in `candidate.satisfied_preconditions`.
3. **Add new needs from the prepended action:**
   - For each precondition, skip if already in `open_preconditions` (dedup).
   - Skip builtin preconditions already satisfied by the **initial state**.
   - Add remaining at `pos = 0`.
   - For each requirement, skip if already in `open_requirements` (dedup).
   - Skip requirements already satisfied by **initial provisions**.
   - Add remaining at `pos = 0`.
4. Merge new bindings into `new_branch.action_bindings`.
5. `recalculate_cost()`.

**Cycle guard:** If `open_requirements.len() > max_depth`, prune (too many accumulated requirements).

**Enqueue** `new_branch` as a fresh `SearchNode`.

**Handle pending:** If `find_candidates` returned a `pending_id`, park the original node.

#### `BranchState::Verifying` (forward validation)

Call `process_simulation(node)`:
1. **Initial-state provisions:** If `simulation_index == 0`, clear any `open_requirements` at `pos == 0` that are satisfied by `initial_provisions`.
2. **Gather bindings** for the current `simulation_index`.
3. **Evaluate preconditions** at this position:
   - Loop through `open_preconditions`. For each matching `pos`:
     - Call `eval_precondition`.
     - If `Ready(true)`, remove it and **return Ready** (one precond per step, for async safety).
     - If `Ready(false)` and `state == Verifying`, return `Invalid`.
     - If `Pending`, return `Pending`.
4. **Simulate the action** at `simulation_index`:
   - Call `simulate_action` with `branch_action_costs` slice (reads/writes cached cost).
   - Update `current_agent`, `current_world`.
   - Clear any downstream `open_requirements` satisfied by this action's `provisions`.
   - Increment `simulation_index`.
   - `recalculate_cost()`.
5. **End of chain:** If `simulation_index == action_chain.len()`:
   - If `open_preconditions` or `open_requirements` remain, return `Invalid`.
   - If `cost < best_cost`, update `best_plan` and `best_cost`.
   - Return `Complete`.

**Process result:**
- `Ready(())` → re-enqueue.
- `Pending(id)` → park node.
- `Invalid` → discard; set debug-tree outcome to `Pruned`.
- `Complete` → if `termination == FirstComplete`, return `Complete(Some(plan))`. Otherwise (BestCost) keep searching.

---

## 4. Candidate Discovery (`find_candidates`)

### Step 1: Gather candidate actions
- For each `open_requirement`, look up actions in `provision_index` that provide a matching `ProvisionKind + name`.
- Also check `FactWildcard` index for `Fact` requirements.
- Always include all `non_wildcard_actions` (they may satisfy preconditions through their effects).

### Step 2: Evaluate each candidate action

**Validity filter:**
- Evaluate each `validity_check` against `initial_state`.
- Builtin checks are evaluated directly.
- Custom checks use the precondition discovery cache (`discovery_precond_results`) and may fire async callbacks.
- If any check fails, skip the action.

**Two code paths depending on whether the action has wildcard provisions:**

#### Path A: Action has wildcard provisions
For each `open_requirement`:
- For each `provision` of the action:
  - If `provision_satisfies_requirement(prov, req)`:
    - Build bindings from the match.
    - Call `get_discovery_result(action_idx, bindings, ctx)`:
      - Checks `discovery_results` cache first.
      - Checks `discovery_pending` (with stale cleanup via `check_pending_or_clean_stale`).
      - If ready, simulates effect from initial state.
      - If pending, records request and returns `Pending(id)`.
    - If discovery is ready, evaluate all `open_preconditions` against the discovery result:
      - Builtin preconditions evaluated directly.
      - Custom preconditions use `discovery_precond_results` cache (may fire async).
    - Create one `Candidate` per `(requirement, provision, discovery_result)` triple.

#### Path B: Action has no wildcard provisions
- Group all matching requirements (one provision can match multiple requirements, but only one match per requirement is recorded).
- Call `get_discovery_result(action_idx, empty_bindings, ctx)`.
- If ready, evaluate all `open_preconditions` against discovery result (same cache/async rules).
- Create a single `Candidate` containing all matched requirements and preconditions.

### Return
```
CandidatesResult {
    ready: Vec<Candidate>,
    pending_id: Option<usize>,  // The most recent pending request ID encountered
}
```

---

## 5. Simulation & Evaluation

### `simulate_action(action_idx, SimArgs) -> StepResult<SimResult>`

**Cost evaluation (cached):**
1. If `branch_action_costs[simulation_index] >= 0`, return cached cost.
2. If `response` contains `CallbackResponse::Float(cost)`, cache and return it.
3. Check global `discovery_costs` cache; if found, write to local cache and return.
4. Otherwise, fire `GetCost` callback and return `Pending(id)`.

**Effect simulation:**
1. If `response` contains `CallbackResponse::UpdatedSnapshots(agent, world)`, return them.
2. Otherwise, fire `ApplyEffect` callback and return `Pending(id)`.
3. If no effect callable, return identity (unchanged agent/world).

### `eval_precondition(spec, agent, world, ctx, response, bindings) -> StepResult<bool>`

- **Builtin:** Evaluate directly (`evaluate_builtin`). Return `Ready(true/false)`.
- **Custom:** If `response` contains `CallbackResponse::Bool(b)`, return `Ready(b)`. Otherwise fire `EvalCustomPrecond` callback and return `Pending(id)`.

---

## 6. Routing Table

| Simulation Result | Engine Action | Godot Main Thread |
| :--- | :--- | :--- |
| `Ready(value)` | Continue processing the current branch/node. | None. |
| `Pending(id)` | Park the node in `parked_nodes[id]`. Return/yield. | Execute the `Callable`, send result to `response_channel`. |
| `Invalid` | Discard node (pruned). | None. |
| `Complete` | If `FirstComplete`, return plan. If `BestCost`, update `best_plan` and keep searching. | None. |

---

## 7. Key Invariants

1. **Preconditions must hold *before* an action runs.** An action's own effect never satisfies its own preconditions during discovery.
2. **Forward validation uses full world context.** `provision_satisfies_requirement` is always called with `Some(&branch.current_world)` during verification.
3. **Branch identity = open needs + bindings + state.** The `fingerprint()` hashes these; concrete binding values are included to prevent false collisions.
4. **One precond per simulation step.** `process_simulation` returns `Ready` after clearing a single precondition, then gets re-queued. This prevents async state accumulation.
5. **Cost is recalculated, not accumulated.** `branch.cost` is always `action_costs.iter().sum()`; no incremental `+=` during simulation.
6. **Discovery caches are per-planning-run.** They live in `SearchContext` and are shared across branches, but not across separate `plan()` invocations.

