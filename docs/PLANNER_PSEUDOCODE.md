# Backward-Chaining GOAP Planner Pseudocode

---

## 1. Overview

The planner is a **hybrid backward-chaining GOAP** search engine:
- **Symbolic layer:** Requirements and Provisions establish causal links.
- **Simulation layer:** GDScript callbacks evaluate costs, effects, and custom preconditions against agent/world snapshots.
- **Search:** Dijkstra (uniform-cost) via a modular `SearchHeuristic` trait. A* is available as a no-op placeholder until an admissible heuristic is designed.
- **Async:** Every Godot callback can yield `Pending`; the engine parks the node and resumes when the response arrives.
- **Multi-goal:** Goals are sorted by reward descending. Before planning, the scheduler checks which goals are already satisfied by the initial state. **If ALL goals are satisfied**, an empty successful plan is returned for the highest-reward one. **If ANY goals are unsatisfied**, all satisfied goals are dropped and the planner searches only the unsatisfied goals. There is no fallback to satisfied goals if the search fails.
- **Threading:** The planner runs on a background thread pool (Rayon). GDScript callables cannot be invoked from background threads, so all cost queries, effect simulations, and custom precondition evaluations are sent as callback requests through a channel to the main thread. The main thread drains these requests each frame and delivers results back to the planner thread.

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
goal_index:           usize                   // Which goal this branch is solving
```

### `SearchNode`
A wrapper that couples a branch with its async state.
```
branch:               PlanBranch
resumed:              bool                    // True if this node just received a callback response
callback_response:    Option<CallbackResponse>// Most recent Godot callback payload
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
cancel_flag:       Arc<AtomicBool>
response_rx:       Receiver<PlannerCallback>
response_tx:       Sender<PlannerCallback>

// Config
heuristic:         Box<dyn SearchHeuristic + Send + Sync>
termination:       TerminationStrategy   // FirstComplete | BestCost
iteration_budget:  usize

// Search State
queue:             BinaryHeap<PriorityNode>
parked_nodes:      HashMap<request_id, Vec<SearchNode>>
visited:           HashMap<SearchFingerprint, cost>
best_plan:         Option<PlanResult>
best_cost:         f64
current_goal_index: usize               // Which goal we're currently solving
tree:              TreeDump
```

### `SearchContext` (Shared, per-planning-run)
```
actions:                   Vec<ActionSpec>
initial_agent:             BlackboardSnapshot
initial_world:             BlackboardSnapshot
initial_provisions:        Vec<ProvisionSpec>        // From agent blackboard + world objects
request_tx:                Sender<CallbackRequest>
engine_response_tx:        Sender<PlannerCallback>

// Discovery Cache (Thread-safe)
discovery_results:         Mutex<HashMap<(action_idx, bindings), DiscoveryResult>>
discovery_costs:           Mutex<HashMap<(action_idx, bindings), f64>>
discovery_pending:         Mutex<HashMap<(action_idx, bindings), request_id>>
discovery_request_map:     Mutex<HashMap<request_id, DiscoveryRequest>>

// Precondition Caching for Discovery (Initial State)
discovery_precond_results: Mutex<HashMap<(action_idx, PreconditionSpec, bindings), bool>>
discovery_precond_pending: Mutex<HashMap<(action_idx, PreconditionSpec, bindings), request_id>>

// Provision Index: maps (ProvisionKind, name) -> action indices that provide it.
provision_index:           HashMap<(ProvisionKind, String), Vec<usize>>
// Action indices that have no wildcard provisions (can be discovered with empty bindings).
non_wildcard_actions:      Vec<usize>
```

### `ActionSpec`
```
name: String
cost_callable_id: Option<usize>
effect_callable_id: Option<usize>
preconditions: Vec<PreconditionSpec>        // Dynamic preconditions for backward chaining
validity_checks: Vec<PreconditionSpec>      // Hard requirements checked against INITIAL state only (during discovery)
requirements: Vec<RequirementSpec>
provisions: Vec<ProvisionSpec>
dependent_object_ids: Vec<i64>              // Object instance IDs this action depends on; if freed, action is excluded
```

### `GoalSpec`
```
name: String
reward: f64
desired_state: Vec<PreconditionSpec>
original_index: usize
```

---

## 3. Search Loop (`step_search`)

### Phase 1: Resume Callbacks
Drain all responses from `response_rx`.
For each `PlannerCallback`:
1. If it is a **Discovery response** (cost/effect/precondition cache):
   - Write the result into the appropriate discovery cache.
   - Remove the request from `discovery_pending` and `discovery_request_map`.
2. If it is a **Precondition discovery response**:
   - Cache the boolean result in `discovery_precond_results`.
   - Remove from `discovery_precond_pending`.
3. Find all **parked nodes** waiting on this `request_id`.
4. If `cancel_flag` is set, clear parked nodes and abort.
5. For each parked node:
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
3. **Compute insertion position** `insert_pos` as the minimum chain position among all consumers (preconditions/requirements) this candidate satisfies. Fallback to `0` for empty chain.
4. Insert `candidate.action_idx` into `new_branch.action_chain` at `insert_pos`.
5. Insert discovery cost into `new_branch.action_costs` at `insert_pos`.
6. Call `new_branch.shift_positions(insert_pos, 1)` — increments all `pos` values in open needs and bindings at or after `insert_pos`.
7. **Update open needs:**
   a. **Remove satisfied requirements:** For each requirement matched by the candidate, do **greedy clearing**: scan ALL `open_requirements` and remove every entry with an identical spec. Collect binding data from the provision that satisfied each requirement. Add provider binding at `insert_pos` and consumer bindings at each cleared consumer's shifted position.
   b. **Remove satisfied preconditions:** Remove all preconditions listed in `candidate.satisfied_preconditions` (indices are pre-shift).
   c. **Add new needs from the inserted action:**
      - For each precondition, skip if already in `open_preconditions` (dedup) or if builtin and already satisfied by the **initial state**. Add remaining at `pos = insert_pos`.
      - For each requirement, skip if already in `open_requirements` (dedup) or already satisfied by **initial provisions**. Add remaining at `pos = insert_pos`.
   d. Merge new bindings into `new_branch.action_bindings`.
   e. `recalculate_cost()`.
8. **Cycle guard:** If `open_requirements.len() > max_depth`, prune (too many accumulated requirements).
9. **Tree:** Add child node to debug tree with updated open needs.
10. **Enqueue** `new_branch` as a fresh `SearchNode`.
11. If **ALL** needs are satisfied, transition to `Verifying` immediately (reset simulation state to initial).

**Handle pending:** If `find_candidates` returned a `pending_id`, park the original node.

#### `BranchState::Verifying` (forward validation)

Call `process_simulation(node)` which delegates to helpers in order:

1. **Initial-state bookkeeping** (`clear_initial_state_requirements`):
   If `simulation_index == 0`, clear any `open_requirements` at `pos == 0` satisfied by `initial_provisions`.

2. **Gather bindings** for the current `simulation_index`.

3. **Re-evaluate action's own preconditions** (`validate_action_against_current_state`):
   For each builtin precondition of the current action that is NOT in `open_preconditions` for this position:
   - Evaluate against `current_agent`/`current_world` snapshots.
   - `Ready(false)` during `Verifying` → `Invalid`.
   - `Pending` → return `Pending`.
   - Custom preconditions are skipped (handled via `open_preconditions`).

4. **Evaluate open preconditions** at this position (`evaluate_open_preconditions_for_position`):
   Loop through `open_preconditions` matching current position:
   - Call `eval_precondition`.
   - If `Ready(true)`, remove it and **return `Ready(())`** (one precond per step, for async safety).
   - If `Ready(false)` during `Verifying`, return `Invalid`.
   - If `Pending`, return `Pending`.

5. **Simulate the current action** (`simulate_and_advance`):
   - If `state == Verifying`, validate all `action.requirements` against current state via `requirement_holds_in_state` (checks bindings, agent properties, world objects). Any failure → `Invalid`.
   - Call `simulate_action` with `SimArgs` (includes local `action_costs` slice for cost caching).
   - On `Ready(res)`: update `current_agent`, `current_world`; call `clear_requirements_from_provisions` to concretize `FactWildcard` provisions using current bindings and clear later `open_requirements`; increment `simulation_index`; `recalculate_cost()`; return `Ready(())`.
   - `Pending`/`Invalid` propagate.

6. **End of chain** (`finalize_verified_branch`):
   If `simulation_index == action_chain.len()`:
   - If any `open_preconditions` or `open_requirements` remain → `Invalid`.
   - If `cost < best_cost`: update `best_plan`, `best_cost`, mark debug tree `Complete`.
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

**Validity filter (against InitialState):**
- Evaluate each `validity_check` against `initial_agent`/`initial_world`.
- Builtin checks evaluated directly.
- **Custom validity checks are NOT evaluated during discovery** — they are only evaluated during forward validation (`validate_action_against_current_state`). This is because validity checks are runtime guards, not planning constraints.
- If any check fails, skip the action.

**Two code paths depending on whether the action has wildcard provisions:**

#### Path A: Action has wildcard provisions (`FactWildcard`)
For each `open_requirement`:
- For each `provision` of the action:
  - If `provision_satisfies_requirement(prov, req, Some(&initial_world))`:
    - Build bindings from the match.
    - Call `get_discovery_result(action_idx, bindings, ctx)`:
      - Checks `discovery_results` cache first.
      - Checks `discovery_pending` (with stale cleanup via `check_pending_or_clean_stale`).
      - If ready, simulates effect from initial state via `simulate_action`.
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

### Stale Pending Cleanup (`check_pending_or_clean_stale`)
When checking `discovery_pending` or `discovery_precond_pending`:
- If the key exists and its `request_id` is still in `discovery_request_map`, the entry is genuinely pending → return the ID.
- If the `request_id` is NOT in `discovery_request_map`, the entry is stale (response was already processed but pending map wasn't cleared) → remove the stale entry and return `None`.

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

### `requirement_holds_in_state(req, agent, world, current_bindings) -> bool`
Validates a requirement against the current simulated state, considering:
- **Bindings:** `BindingExists`, `BindingEquals`, `BindingInSet` check `current_bindings` first (any-of semantics for multiple bindings with the same name).
- **Agent properties:** Fallback for bindings not yet materialized in snapshots.
- **World objects:** For `BindingInSet` with `ObjectRef`, checks world object groups. For `Fact { fact_name: "at_target", args: [target] }`, compares agent's location object position against target's position from world. Falls back to bindings if no location state available.

### `provision_satisfies_requirement(prov, req, world) -> bool`
Symbolic matching used during candidate discovery and forward validation:
- `Binding` ↔ `BindingExists`/`BindingEquals`/`BindingInSet` (with world context for `BindingInSet`).
- `Fact` ↔ `Fact` (exact name and args match).
- `FactWildcard` ↔ `Fact` (name match only; args ignored).
- `world` context is `Some` during forward validation, `None` during discovery (except initial-world discovery which passes `Some(&initial_world)`).

### `concretize_wildcard_provision(prov, current_bindings) -> ProvisionSpec`
If `prov` is `FactWildcard { fact_name }` and a binding exists for `fact_name` in `current_bindings`, returns `Fact { fact_name, args: binding_values }`. Otherwise returns `prov` unchanged.

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
7. **Action insertion position = earliest consumer.** Predecessor actions are inserted immediately before the first action that consumes their provision/precondition, not at the front of the chain. Positions of subsequent actions are shifted accordingly.
8. **Goal handling.** Goals are sorted by reward descending. Before planning, the scheduler checks which goals are already satisfied by the initial state. **If ALL goals are satisfied**, an empty successful plan is returned for the highest-reward one. **If ANY goals are unsatisfied**, ALL satisfied goals are dropped and the planner searches only the unsatisfied goals. There is no fallback to satisfied goals if the search fails.
9. **Binding injection.** All callbacks (`GetCost`, `ApplyEffect`, `EvalCustomPrecond`) receive `agent` and `world` snapshots with bindings already injected into the agent blackboard. The callback contract is always `(agent, world)` — 2 arguments.
10. **Greedy requirement clearing.** When an action satisfies a requirement, ALL identical open requirements in the branch are cleared by that one action (single provider for multiple consumers).
11. **Stale pending cleanup.** Discovery and precondition pending maps are defended against stale entries: if a request ID no longer exists in `discovery_request_map`, the pending entry is removed and the action is re-evaluated.
12. **Validity checks are initial-state only.** `validity_checks` are evaluated against the initial agent/world snapshots during candidate discovery. Custom validity checks are deferred to forward validation; they do not fire async during discovery.
13. **Non-wildcard actions assume no binding variation.** Actions without `FactWildcard` provisions are discovered once with empty bindings. Their cost/effect is computed generically.

---

## 8. Data Specifications (Send-Safe Mirrors)

### `PreconditionSpec`
```
Builtin { target: Agent|WorldState, operation: HasProperty|Equal|NotEqual|GreaterThan|..., property_name: String, value: Option<VariantSnapshot> }
Custom  { callable_id: usize, dependent_object_ids: Vec<i64> }
```

### `RequirementSpec`
```
BindingExists { binding_name: String }
BindingEquals { binding_name: String, value: VariantSnapshot }
BindingInSet  { binding_name: String, set_name: String }
Fact          { fact_name: String, args: Vec<VariantSnapshot> }
```

### `ProvisionSpec`
```
Binding       { binding_name: String, value: VariantSnapshot }
Fact          { fact_name: String, args: Vec<VariantSnapshot> }
FactWildcard  { fact_name: String }
```

### `ActionSpec`
```
name: String
cost_callable_id: Option<usize>
effect_callable_id: Option<usize>
preconditions: Vec<PreconditionSpec>
validity_checks: Vec<PreconditionSpec>
requirements: Vec<RequirementSpec>
provisions: Vec<ProvisionSpec>
dependent_object_ids: Vec<i64>
```

### `GoalSpec`
```
name: String
reward: f64
desired_state: Vec<PreconditionSpec>
original_index: usize
```

### `CallbackKind` (Planner → Main Thread)
```
GetCost      { agent: BlackboardSnapshot, world: BlackboardSnapshot }
ApplyEffect  { agent: BlackboardSnapshot, world: BlackboardSnapshot }
EvalCustomPrecond { agent: BlackboardSnapshot, world: BlackboardSnapshot }
```
Bindings are pre-injected into `agent` before the call.

### `CallbackResponse` (Main Thread → Planner)
```
Float(f64)                    // GetCost
Bool(bool)                    // EvalCustomPrecond
UpdatedSnapshots(agent, world) // ApplyEffect
```

### `PlanResult` (Final Output)
```
success: bool
action_chain: Vec<i64>          // Action indices (for Godot)
total_cost: f64
goal_index: i64
deferred_action_indices: Vec<i64>  // Currently unused, reserved
action_bindings: Vec<(i64, String, Vec<VariantSnapshot>)>  // pos, name, values
```

### `DiscoveryRequest`
```
Simulation(action_idx, bindings)
Precondition(action_idx, PreconditionSpec, bindings)
```

### `DiscoveryResult`
```
agent: BlackboardSnapshot
world: BlackboardSnapshot
cost: f64
```

### `CallbackRequest` (Planner → Main Thread)
```
request_id: usize
callable_id: usize
kind: CallbackKind
bindings: Vec<(String, Vec<VariantSnapshot>)>
response_tx: Sender<PlannerCallback>
```

### `PlannerCallback` (Main Thread → Planner)
```
request_id: usize
response: CallbackResponse
```

### `PlannerRunResult`
```
Complete(Option<PlanResult>)
Pending(request_id)
```

### `TerminationStrategy`
```
FirstComplete
BestCost
```

### `BranchState`
```
Searching
Verifying
```

### `ProvisionKind`
```
Binding
Fact
FactWildcard
```

---

## 9. Scheduler Integration (GDScript Entry Point)

The `GdPAIPlanScheduler` (GDScript-facing autoload) manages the planning lifecycle:

### `submit_plan(agent, agent_bb, world_bb, actions, goals, max_recursion, iteration_budget)`
1. Cancel any existing jobs for the same agent.
2. Snapshot agent/world blackboards into `BlackboardSnapshot`.
3. Extract `initial_provisions` from agent properties + world objects (objects with `provides` fact).
4. Build `ActionSpec` and `GoalSpec` from GDScript dictionaries, registering callables.
5. **Pre-filter goals:** Evaluate each goal's `desired_state` against initial state using `PreconditionHandler`. Track highest-reward satisfied goal.
6. Drop goals already satisfied by initial state. **If ALL goals were satisfied**, record `satisfied_goal_index` and submit empty goal list to planner.
7. Sort remaining goals by reward descending.
8. Build `provision_index` and `non_wildcard_actions`.
9. Create `SearchContext` with all caches and channels.
10. Create `PlannerEngine` with Dijkstra heuristic, BestCost termination, iteration budget.
11. Spawn engine on Rayon thread pool via `run_job_step`.
12. Store `ActiveJobHandle` with engine, channels, cancel flag, goal list, `satisfied_goal_index`.

### `process_callbacks()` (Called each frame from GDScript)
1. **Recover engines:** Drain `result_rx` for completed/paused engines. Store engine back in handle.
   - If `Complete(Some(plan))`: If `satisfied_goal_index >= 0` and plan failed, convert to empty success plan for that goal. Call `agent._on_plan_ready(dict)`.
   - If `Pending(id)`: Record `pending_request_id`.
2. **Process Godot callbacks:** Drain `request_rx` for each active job.
   - Dispatch callable with `kind` (GetCost/ApplyEffect/EvalCustomPrecond).
   - Inject `bindings` into agent blackboard before call.
   - Send `PlannerCallback` back via `response_tx`.
   - Track completed request IDs per job.
3. **Resume ready engines:** For each job not done:
   - If `pending_request_id == 0` (budget yield) → ready.
   - If `pending_request_id > 0` → ready only if response received this frame OR already processed in previous frame.
   - Resume via `run_job_step` with cloned goals and engine.
4. Clean up finished jobs after one frame grace period.

### `cancel_agent_jobs(agent)` / `cancel_all_jobs()` / `clear_active_jobs()`
Set `cancel_flag` on matching jobs. Engine checks flag at loop start and returns `Complete(None)`.

---

## 10. Debug Tree (`TreeDump`)

Enabled when log level ≥ Debug. Flat ID-based structure:

```
GoalAttempt {
    goal_name, goal_reward, goal_preconditions, already_satisfied,
    root_id, success, plan_actions, plan_cost
}

TreeNode {
    action_name: Option<String>      // None for root
    estimated_cost, accumulated_cost: f64
    open_preconditions: Vec<String>
    open_requirements: Vec<String>
    satisfied_preconditions: Vec<String>
    satisfied_requirements: Vec<String>
    excluded_actions: Vec<ExcludedAction>  // {action_name, reason}
    outcome: NodeOutcome
    forward_validation: Vec<FwdStep>
    children: Vec<usize>
}

NodeOutcome:
    Expanded
    Pruned { reason }
    DeadEnd
    Complete { chain_len, total_cost, fwd_ok }
```

Methods: `begin_goal`, `add_root`, `add_child`, `set_outcome`, `exclude_action`, `end_goal`, `format()`.

---

(End of document)