# Backward-Chaining GOAP Planner — Algorithmic Pseudocode

> **Source:** `addons/GdPlanningAI/rust/src/planner/` + `scheduler.rs` (audited October 2026) **Purpose:** Algorithmic reference — not a type spec. See source for exact types.

______________________________________________________________________

## 1. Architecture

| Layer          | Role                                                                                                                                                                                           |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Symbolic**   | Requirements ↔ Provisions (causal links, backward chaining)                                                                                                                                    |
| **Simulation** | GDScript callbacks for costs, effects, custom preconditions                                                                                                                                    |
| **Search**     | Dijkstra (uniform-cost) via `SearchHeuristic`; A\* placeholder                                                                                                                                 |
| **Async**      | Every callback yields `Pending(id)`; node parked, resumed on response. Budget or time-slice exhaustion yields `Pending(0)`, resumed unconditionally                                            |
| **Threading**  | Planner on Rayon pool; callbacks on Godot main thread. Godot values are converted to Send-safe snapshots on the main thread; the engine is moved (never shared) between worker and main thread |
| **Multi-Goal** | Goals sorted by reward desc. Pre-filter: satisfied goals dropped. If ALL satisfied → empty plan for highest-reward. If ANY unsatisfied → search only unsatisfied.                              |

______________________________________________________________________

## 2. Key Types (Conceptual)

```
ActionSpec     { name, cost_fn?, effect_fn?, preconditions[], validity_checks[], requirements[], provisions[], dependent_objects[] }
GoalSpec       { name, reward, desired_state[PreconditionSpec], original_index }
Precondition   { Builtin(target, op, property, value?) | Custom(callable_id, deps[]) }
Requirement    { BindingExists(name) | BindingEquals(name,val) | BindingInSet(name,set) | Fact(name,args[]) }
Provision      { Binding(name,val) | Fact(name,args[]) | FactWildcard(name) }  // wildcard matches any Fact(name,_)

BranchState    = Searching | Verifying
PlanBranch     { action_chain[], action_costs[], action_bindings[(pos,name,values)], open_preconditions[(pos,spec)], open_requirements[(pos,spec)], state, simulation_index, current_agent, current_world, cost, tree_node_id, goal_index }
               // pos = chain index of the consuming action; a need must hold in the state just before that action.
               // pos == action_chain.len means the final state, where goal preconditions end up.
               // action_costs is seeded from the discovery cost on insertion; cost = sum(action_costs)
SearchNode     { branch, resumed, callback_response, expanded_candidates[(action_idx,bindings)] }
SearchContext  { actions[], initial_agent, initial_world, initial_provisions[], request_tx, engine_response_tx, caches..., provision_index[(kind,name)→action_idxs], non_wildcard_actions[] }
```

**Caches (per planning run, thread-safe):**

- `discovery_results[(action_idx,bindings)] → {agent,world,cost}`
- `discovery_costs[(action_idx,bindings)] → f64`
- `discovery_precond_results[(action_idx,spec,bindings)] → bool`
- Pending maps (`discovery_pending`, `discovery_precond_pending`) with stale-entry cleanup via `discovery_request_map` (request id → what it was for)
- On the engine (verification): `step_cache[(pre_agent,pre_world,action_idx,bindings)] → shared post-state`, keyed by snapshot identity, with `step_pending` / `step_requests` for in-flight effects

______________________________________________________________________

## 3. Search Loop (`step_search`)

```
function step_search(goals):
    if cancel_flag: return Complete(None)

    // Phase 1: Resume callbacks
    while response = response_rx.try_recv():
        if cancel_flag: parked_nodes.clear(); return Complete(None)
        if DiscoveryResponse: write into caches, remove from pending maps
        if step request: step_cache[key] = snapshots, remove from step_pending
        for parked_node waiting on request_id:
            parked_node.resumed = true; parked_node.callback_response = response
            enqueue(parked_node)

    // Phase 2: Main loop
    iterations = 0
    while node = queue.pop():
        iterations += 1
        if BestCost and heuristic.prune(node.priority, best_cost): re-enqueue; break
        if iterations > budget: enqueue(node); return Pending(0)
        if cancel_flag: return Complete(None)

        if not resumed and state == Searching:
            fp = fingerprint(branch)
            if visited[fp] <= branch.cost: continue
            visited[fp] = branch.cost

        if resumed: expanded_candidates.clear()
        resumed = false
        if branch.cost >= best_cost: continue

        // Time slice (0 disables); the clock is only read every 128 pops
        if time_slice_ms > 0 and iterations % 128 == 0 and elapsed >= time_slice:
            enqueue(node); return Pending(0)

        match branch.state:

            Searching:
                if open_preconditions.empty and open_requirements.empty:
                    state = Verifying; simulation_index = 0; reset to initial state; enqueue; continue
                if action_chain.len >= max_depth: continue

                candidates = find_candidates(branch, ctx)

                for cand in candidates.ready:
                    if (cand.action_idx, cand.bindings) in expanded_candidates: continue
                    expanded_candidates.push((cand.action_idx, cand.bindings))

                    new_branch = branch.clone()
                    insert_pos = min_position(cand.satisfied_preconditions, cand.satisfied_requirements)
                    // (falls back to 0 when no consumer sits before the end of the chain)

                    // Insert action + cost
                    new_branch.action_chain.insert(insert_pos, cand.action_idx)
                    new_branch.action_costs.insert(insert_pos, discovery_cost(cand))
                    new_branch.shift_positions(insert_pos, 1)

                    // Remove satisfied (greedy: ALL identical requirements)
                    for (req_idx, req, prov) in cand.satisfied_requirements:
                        remove ALL open_requirements where spec == req
                        add provider binding at insert_pos
                        add consumer bindings at each removed position
                    remove open_preconditions at cand.satisfied_precondition_indices

                    // Add new action's needs (dedup + initial-state filter)
                    new_preconditions = action.preconditions
                        .filter(not in open_preconditions)
                        .filter(builtin AND satisfied by initial_state → skip)
                    new_requirements = action.requirements
                        .filter(not in open_requirements)
                        .filter(satisfied by initial_provisions → skip)
                    add at insert_pos; merge bindings

                    new_branch.recalculate_cost()

                    // Fully satisfied → verify immediately
                    if new_branch.open_preconditions.empty and new_branch.open_requirements.empty:
                        new_branch.state = Verifying; simulation_index = 0; reset to initial

                    // Cycle guard
                    if new_branch.open_requirements.len > max_depth: mark Pruned; continue

                    enqueue(new_branch)

                if candidates.pending_id: park original node

            Verifying:
                result = process_simulation(node)
                Ready → enqueue
                Pending(id) → park
                Invalid → discard
                Complete:
                    if FirstComplete and best_plan: return Complete(plan)
                    else: discard, keep searching (BestCost)

    // Exhaustion / goal sequencing
    if parked_nodes not empty: return Pending(any_parked_id)
    if best_plan exists and empty and more goals: reset best_plan/best_cost; advance to next goal; return Pending(0)
    if no best_plan and more goals: advance to next goal; return Pending(0)
    return Complete(best_plan or failed PlanResult)   // Complete(None) is reserved for cancellation
```

______________________________________________________________________

## 4. Candidate Discovery (`find_candidates`)

```
function find_candidates(branch, ctx) → {ready[], pending_id?}:
    candidates = []; some_pending = false; last_pending = 0
    candidate_actions = BTreeSet()

    // Actions providing open requirements
    for (_, req) in open_requirements:
        candidate_actions += provision_index[kind(req), req.name]
        if req is Fact: candidate_actions += provision_index[FactWildcard, req.fact_name]

    // Non-wildcard actions (may satisfy preconditions via effects)
    candidate_actions += non_wildcard_actions

    for idx in candidate_actions:
        action = ctx.actions[idx]

        // Validity filter (initial state)
        for check in action.validity_checks:
            if Builtin: if not evaluate(initial_agent, initial_world): skip
            else:  // Custom
                if cached = discovery_precond_results[idx, check, []]: if not cached: skip else continue
                if pending = check_pending_or_clean_stale(...): some_pending=true; last_pending=pending; skip
                eval_precondition(check, initial_agent, initial_world, ctx, None, [])
                some_pending=true; last_pending=request_id; skip

        // Discovery
        has_wildcard = action.provisions.any(FactWildcard)

        if has_wildcard:
            // Per requirement × per matching provision
            for (req_idx, (_, req)) in open_requirements.enumerate():
                for prov in action.provisions:
                    if provision_satisfies_requirement(prov, req, initial_world):
                        bindings = binding_values(prov, req)
                        res = lookup_or_start_discovery(idx, bindings, ctx)  // result cache → pending map → simulate (cost, then effect)
                        if res is Pending: some_pending=true; last_pending=res.id; continue

                        // Evaluate open_preconditions against res
                        satisfied_pre = []
                        for (pre_idx, (_, pre)) in open_preconditions.enumerate():
                            if Builtin: if pre.evaluate(res.agent, res.world): satisfied_pre.push(pre_idx)
                            else:
                                if cached = discovery_precond_results[idx, pre, bindings]: if cached: push
                                else if pending = check_pending_or_clean_stale(...): some_pending=true; break
                                else: eval_precondition(pre, res.agent, res.world, ctx, None, bindings); some_pending=true; break
                        if no precondition check for this candidate is pending:
                            candidates.push({idx, [(req_idx,req,prov)], satisfied_pre, bindings})

        else:
            // Single discovery with empty bindings
            satisfied_reqs = []; matched = BTreeSet()
            for (req_idx, (_, req)) in open_requirements.enumerate():
                for prov in action.provisions:
                    if provision_satisfies_requirement(prov, req, initial_world):
                        if matched.insert(req_idx): satisfied_reqs.push((req_idx,req,prov))
                        break

            res = lookup_or_start_discovery(idx, [], ctx)
            if res is Pending: some_pending=true; last_pending=res.id; continue

            satisfied_pre = evaluate open_preconditions same as above
                // a custom check that is still pending marks the node for parking, but the
                // candidate is still emitted, just without credit for that precondition
            if satisfied_reqs not empty OR satisfied_pre not empty:
                candidates.push({idx, satisfied_reqs, satisfied_pre, []})

    return {ready: candidates, pending_id: some_pending ? last_pending : None}
```

**Stale cleanup:** `check_pending_or_clean_stale(pending_map, request_map, key)` — if `request_map` lacks the ID, remove stale entry.

**Wildcard binding:** for a `FactWildcard` provision matched against `Fact(n, args)`, `binding_values_from_match` yields `[(n, args)]`; that is the `bindings` the action is discovered, simulated and cached with, and what `insert_action_at` records at the provider's position and at each consumer position.

______________________________________________________________________

## 5. Forward Verification (`process_simulation`)

```
function process_simulation(node) → StepResult:
    branch = node.branch
    if simulation_index == 0: clear_initial_state_requirements(branch)  // pos-0 reqs satisfied by initial provisions

    bindings = branch.bindings_at(simulation_index)

    // 1. Action's own builtin preconditions (not in open_preconditions)
    if simulation_index < action_chain.len:
        for pre in action.preconditions:
            if pre in open_preconditions at simulation_index: continue
            if pre is Custom: continue  // handled via open_preconditions
            match eval_precondition(pre, current_agent, current_world, ctx, callback_response, bindings):
                Ready(true) → continue
                Ready(false) → Invalid (if Verifying)
                Pending(id) → Pending(id)
                Invalid → Invalid

    // 2. One open precondition at this position
    if result = evaluate_open_preconditions(branch, bindings, callback_response):
        if Ready: callback_response = None
        return result

    // 3. Simulate action or finalize
    if simulation_index < action_chain.len:
        // Verify requirements against current state
        if Verifying:
            for req in action.requirements:
                if not requirement_holds_in_state(req, current_agent, current_world, bindings): return Invalid

        key = (current_agent, current_world, action_idx, bindings)
        if cost known:
            if step_cache[key]: take it, skip simulate_action (no callback)
            elif no response and step_pending[key] = id: return Pending(id)  // share the request
        match simulate_action(action_idx, {agent, world, ctx, response, branch_action_costs[simulation_index], bindings}):
            Ready(res):
                current_agent, current_world = no effect ? unchanged : step_cache.get_or_insert(key, res)
                clear_requirements_from_provisions(branch, action_idx, bindings)  // concretize wildcards
                simulation_index++; recalculate_cost(); callback_response = None
                return Ready(())
            Pending(id) → if cost known: step_pending[key] = id; Pending(id)
            Invalid → Invalid

    // 4. End of chain
    if open_preconditions not empty OR open_requirements not empty: return Invalid
    if branch.cost < best_cost: best_cost = branch.cost; best_plan = PlanResult from branch; mark Complete
    return Complete
```

______________________________________________________________________

## 6. Simulation & Evaluation

**Cost (3-tier cache):**

```
(branch_action_costs is seeded with the discovery cost on insertion, so verification normally
 hits the first line; discovery itself simulates against the *initial* state)
branch_action_costs[sim_idx] ≥ 0  → cached
response has Float                  → use & cache
global discovery_costs has key      → use & cache locally
else                                → fire GetCost callback → Pending
(no cost_callable → 1.0; an infinite cost → Invalid, before any effect is requested)
```

**Effect:**

```
has effect_callable:
    response has UpdatedSnapshots → Ready(agent, world, cost)
    else → fire ApplyEffect → Pending
no effect_callable → Ready(identity, cost)
```

**`eval_precondition(spec, agent, world, ctx, response, bindings)`**

```
Builtin → Ready(evaluate_builtin(agent, world))
Custom  → response has Bool ? Ready(bool) : fire EvalCustomPrecond → Pending
```

**`requirement_holds_in_state(req, agent, world, bindings)`** | Requirement | Check | |-------------|-------| | `BindingExists(n)` | bindings[n] non-empty OR agent.properties[n] non-null | | `BindingEquals(n,v)` | bindings[n] == v OR agent.properties[n] == v | | `BindingInSet(n,set)` | first value of a bindings[n] entry is an ObjectRef to a world object in group `set` OR agent.property same | | `Fact(at_target, [target])` | agent.location.position == target.position (from world); fallback to bindings | | other `Fact` | assumed true (covered by preconditions) |

**`provision_satisfies_requirement(prov, req, world_opt)`** | Provision → Requirement | Match | |------------------------|-------| | Binding(n,v) → BindingExists(n) | n == n | | Binding(n,v) → BindingEquals(n,v) | n==n ∧ v==v | | Binding(n,obj) → BindingInSet(n,set) | n match ∧ world_opt has obj in set | | Fact(n,args) → Fact(n,args) | n match ∧ (args empty ∨ args==args) | | FactWildcard(n) → Fact(n,\_) | n match |

______________________________________________________________________

## 7. Callback Protocol

```
Planner → Main:  CallbackRequest { request_id, callable_id, kind, bindings[], response_tx }
  kind ∈ { GetCost(agent,world), ApplyEffect(agent,world), EvalCustomPrecond(agent,world) }

Main → Planner:  PlannerCallback { request_id, response }
  response ∈ { Float(f64), Bool(bool), UpdatedSnapshots(agent,world) }

Bindings injected into agent.properties only (not world).
```

______________________________________________________________________

## 8. Scheduler (`GdPAIPlanScheduler`)

```
submit_plan(agent, agent_bb, world_bb, actions, goals, max_depth, budget, time_slice_ms):
    cancel existing jobs for agent
    snap_agent = snapshot(agent_bb); snap_world = snapshot(world_bb)
    initial_provisions = extract(agent) + extract(world)

    action_specs = build(actions)  // registers callables → IDs
    goal_specs = build(goals)

    // Pre-filter satisfied goals (main thread, PreconditionHandler on the live blackboards)
    satisfied = highest_reward goal where desired_state all true in initial state
    goal_specs = goal_specs.filter(not satisfied)
    satisfied_goal_index = goal_specs.empty ? satisfied : -1
    goal_specs.sort_by(reward desc)

    build provision_index + non_wildcard_actions
    ctx = SearchContext{...}
    engine = PlannerEngine(ctx, max_depth, cancel_flag)
        .heuristic(Dijkstra).termination(BestCost).budget(budget).time_slice(time_slice_ms)
    spawn first step on Rayon pool (the worker owns the engine until it sends it back)

process_callbacks()  // each frame:
    1. Recover engines from workers:
       Complete(plan) → if empty and satisfied_goal_index≥0: plan = empty_success(satisfied)
                       agent._on_plan_ready(plan)
       Pending(id) → job.pending_request_id = id

    2. Process Godot callbacks:
       dispatch callable with bindings injected → send response

    3. Resume ready engines:
       ready if (pending_id==0) OR (response this frame) OR (response already processed)
       run_job_step(goals, engine)

    4. Cleanup finished jobs (one frame grace); reaped engines are freed on a
       background thread, not the main thread
```

______________________________________________________________________

## 9. Key Invariants

1. Preconditions hold *before* action runs (own effect never satisfies own preconditions).
1. Forward validation uses full world context (`Some(&branch.current_world)`).
1. Branch identity = goal + open needs + bindings + state (fingerprint hashes all).
1. One precondition per simulation step (re-queue after each).
1. Cost = sum(action_costs) always (recalculated, not accumulated).
1. Discovery caches per planning run (shared across branches).
1. Insertion position = earliest consumer (predecessor before first consumer).
1. Goal handling: satisfied goals are dropped and the rest searched by reward (section 1, `submit_plan`).
1. Bindings injected into agent only.
1. Greedy requirement clearing: one action clears ALL identical open requirements.
1. Stale pending cleanup via `request_map`.
1. Validity checks during discovery: builtin sync, custom async.
1. Custom preconditions evaluated during discovery (against initial-state sim) + tracked for verification.
1. Non-wildcard actions discovered once with empty bindings.
1. Snapshot conversion (Variant ↔ `VariantSnapshot`, blackboard ↔ `BlackboardSnapshot`) happens on the main thread only; the engine is owned by one thread at a time.
1. Costs are evaluated against the initial state during discovery and reused in verification.

______________________________________________________________________

## 10. Termination

| Strategy        | Behavior                                                   |
| --------------- | ---------------------------------------------------------- |
| `FirstComplete` | Return first valid plan                                    |
| `BestCost`      | Exhaust search (prune via `heuristic.prune_threshold_met`) |

Default: **BestCost** + Dijkstra.

______________________________________________________________________

## 11. Logging

```
log levels: 0 Error, 1 Warn, 2 Info (default), 3 Debug

submit_plan(...) [Info]:
    log "submit_plan: agent goals=[...] actions=N"

process_callbacks()  // each frame:
    ... (steps 1-4 as above)
    stalled (pending id with no response): log "NOT resuming" once per pending id [Debug]

plan completion [Info]:
    log "RESULT goal='...' success=... actions=... cost=... Branches: ... | Time: ...ms"
    (skipped when there were no goals to search)
    full tree never auto-prints; fetch via get_debug_tree(agent) [Debug records it]

per-iteration detail (candidate lists, simulation payloads,
callback handshakes) is not logged: the tree carries the same
information with node context, and verification requirement
failures are recorded as FWD [FAIL] steps on their node
```

______________________________________________________________________

*End of Algorithmic Pseudocode*
