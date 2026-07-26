# Backward-Chaining GOAP Planner — Algorithmic Pseudocode

> **Source:** `addons/GdPlanningAI/rust/src/planner/` + `scheduler.rs` (audited July 2026)
> **Purpose:** Algorithmic reference — not a type spec. See source for exact types.

---

## 1. Architecture

| Layer | Role |
|-------|------|
| **Symbolic** | Requirements ↔ Provisions (causal links, backward chaining) |
| **Simulation** | GDScript callbacks for costs, effects, custom preconditions |
| **Search** | Dijkstra (uniform-cost) via `SearchHeuristic`; A* placeholder |
| **Async** | Every callback yields `Pending(id)`; node parked, resumed on response |
| **Threading** | Planner on Rayon pool; callbacks on Godot main thread |
| **Multi-Goal** | Goals sorted by reward desc. Pre-filter: satisfied goals dropped. If ALL satisfied → empty plan for highest-reward. If ANY unsatisfied → search only unsatisfied. |

---

## 2. Key Types (Conceptual)

```
ActionSpec     { name, cost_fn?, effect_fn?, preconditions[], validity_checks[], requirements[], provisions[], dependent_objects[] }
GoalSpec       { name, reward, desired_state[PreconditionSpec], original_index }
Precondition   { Builtin(target, op, property, value?) | Custom(callable_id, deps[]) }
Requirement    { BindingExists(name) | BindingEquals(name,val) | BindingInSet(name,set) | Fact(name,args[]) }
Provision      { Binding(name,val) | Fact(name,args[]) | FactWildcard(name) }  // wildcard matches any Fact(name,_)

BranchState    = Searching | Verifying
PlanBranch     { action_chain[], action_costs[], action_bindings[], open_preconditions[(pos,spec)], open_requirements[(pos,spec)], state, sim_index, current_agent, current_world, cost, tree_node_id, goal_index }
SearchNode     { branch, resumed, callback_response, expanded_candidates[(action_idx,bindings)] }
SearchContext  { actions[], initial_agent, initial_world, initial_provisions[], request_tx, response_tx, caches..., provision_index[(kind,name)→action_idxs], non_wildcard_actions[] }
```

**Caches (per planning run, thread-safe):**
- `discovery_results[(action_idx,bindings)] → {agent,world,cost}`
- `discovery_costs[(action_idx,bindings)] → f64`
- `discovery_precond_results[(action_idx,spec,bindings)] → bool`
- Pending maps with stale-entry cleanup via `request_map`

---

## 3. Search Loop (`step_search`)

```
function step_search(goals):
    // Phase 1: Resume callbacks
    while response = response_rx.try_recv():
        if DiscoveryResponse: write into caches, remove from pending maps
        for parked_node waiting on request_id:
            parked_node.resumed = true; parked_node.callback_response = response
            enqueue(parked_node)

    // Phase 2: Main loop
    iterations = 0
    while node = queue.pop():
        if BestCost and heuristic.prune(node.priority, best_cost): re-enqueue; break
        if cancel_flag: return Complete(None)
        if iterations > budget: enqueue(node); return Pending(0)

        if not resumed and state == Searching:
            fp = fingerprint(branch)
            if visited[fp] <= branch.cost: continue
            visited[fp] = branch.cost

        if resumed: expanded_candidates.clear()
        resumed = false
        if branch.cost >= best_cost: continue

        match branch.state:

            Searching:
                if open_preconditions.empty and open_requirements.empty:
                    state = Verifying; sim_index = 0; reset to initial state; enqueue; continue
                if action_chain.len >= max_depth: continue

                candidates = find_candidates(branch, ctx)

                for cand in candidates.ready:
                    if (cand.action_idx, cand.bindings) in expanded_candidates: continue
                    expanded_candidates.push((cand.action_idx, cand.bindings))

                    new_branch = branch.clone()
                    insert_pos = min_position(cand.satisfied_preconditions, cand.satisfied_requirements)

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

                    // Cycle guard
                    if new_branch.open_requirements.len > max_depth: mark Pruned; continue

                    // Fully satisfied → verify immediately
                    if new_branch.open_preconditions.empty and new_branch.open_requirements.empty:
                        new_branch.state = Verifying; sim_index = 0; reset to initial

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
    if parked_nodes not empty: return Pending(first_pending_id)
    if best_plan exists and empty and more goals: advance to next goal; return Pending(0)
    return Complete(best_plan or failure)
```

---

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
                    if provision_satisfies(prov, req, initial_world):
                        bindings = binding_values(prov, req)
                        res = get_discovery(idx, bindings, ctx)  // 3-tier cost cache
                        if res is Pending: some_pending=true; last_pending=res.id; continue

                        // Evaluate open_preconditions against res
                        satisfied_pre = []
                        for (pre_idx, (_, pre)) in open_preconditions.enumerate():
                            if Builtin: if pre.evaluate(res.agent, res.world): satisfied_pre.push(pre_idx)
                            else:
                                if cached = discovery_precond_results[idx, pre, bindings]: if cached: push
                                else if pending = check_pending_or_clean_stale(...): some_pending=true; break
                                else: eval_precondition(pre, res.agent, res.world, ctx, None, bindings); some_pending=true; break
                        if not some_pending:
                            candidates.push({idx, [(req_idx,req,prov)], satisfied_pre, bindings})

        else:
            // Single discovery with empty bindings
            satisfied_reqs = []; matched = BTreeSet()
            for (req_idx, (_, req)) in open_requirements.enumerate():
                for prov in action.provisions:
                    if provision_satisfies(prov, req, initial_world):
                        if matched.insert(req_idx): satisfied_reqs.push((req_idx,req,prov))
                        break

            res = get_discovery(idx, [], ctx)
            if res is Pending: some_pending=true; last_pending=res.id; continue

            satisfied_pre = evaluate open_preconditions same as above
            if satisfied_reqs not empty OR satisfied_pre not empty:
                candidates.push({idx, satisfied_reqs, satisfied_pre, []})

    return {ready: candidates, pending_id: some_pending ? last_pending : None}
```

**Stale cleanup:** `check_pending_or_clean_stale(pending_map, request_map, key)` — if `request_map` lacks the ID, remove stale entry.

---

## 5. Forward Verification (`process_simulation`)

```
function process_simulation(node) → StepResult:
    branch = node.branch
    if sim_index == 0: clear_initial_state_requirements(branch)  // pos-0 reqs satisfied by initial provisions

    bindings = branch.bindings_at(sim_index)

    // 1. Action's own builtin preconditions (not in open_preconditions)
    if sim_index < action_chain.len:
        for pre in action.preconditions:
            if pre in open_preconditions at sim_index: continue
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
    if sim_index < action_chain.len:
        // Verify requirements against current state
        if Verifying:
            for req in action.requirements:
                if not requirement_holds(req, current_agent, current_world, bindings): return Invalid

        match simulate_action(action_idx, {agent, world, ctx, response, branch_action_costs[sim_index], bindings}):
            Ready(res):
                current_agent = res.agent; current_world = res.world
                clear_requirements_from_provisions(branch, action_idx, bindings)  // concretize wildcards
                sim_index++; recalculate_cost(); callback_response = None
                return Ready(())
            Pending(id) → Pending(id)
            Invalid → Invalid

    // 4. End of chain
    if open_preconditions not empty OR open_requirements not empty: return Invalid
    if branch.cost < best_cost: best_cost = branch.cost; best_plan = PlanResult from branch; mark Complete
    return Complete
```

---

## 6. Simulation & Evaluation

**Cost (3-tier cache):**
```
branch_action_costs[sim_idx] ≥ 0  → cached
response has Float                  → use & cache
global discovery_costs has key      → use & cache locally
else                                → fire GetCost callback → Pending
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

**`requirement_holds(req, agent, world, bindings)`**
| Requirement | Check |
|-------------|-------|
| `BindingExists(n)` | bindings[n] non-empty OR agent.properties[n] non-null |
| `BindingEquals(n,v)` | bindings[n] == v OR agent.properties[n] == v |
| `BindingInSet(n,set)` | bindings[n] is ObjectRef in world.objects[set] OR agent.property same |
| `Fact(at_target, [target])` | agent.location.position == target.position (from world); fallback to bindings |

**`provision_satisfies(prov, req, world_opt)`**
| Provision → Requirement | Match |
|------------------------|-------|
| Binding(n,v) → BindingExists(n) | n == n |
| Binding(n,v) → BindingEquals(n,v) | n==n ∧ v==v |
| Binding(n,obj) → BindingInSet(n,set) | n match ∧ world_opt has obj in set |
| Fact(n,args) → Fact(n,args) | n match ∧ (args empty ∨ args==args) |
| FactWildcard(n) → Fact(n,_) | n match |

---

## 7. Callback Protocol

```
Planner → Main:  CallbackRequest { request_id, callable_id, kind, bindings[], response_tx }
  kind ∈ { GetCost(agent,world), ApplyEffect(agent,world), EvalCustomPrecond(agent,world) }

Main → Planner:  PlannerCallback { request_id, response }
  response ∈ { Float(f64), Bool(bool), UpdatedSnapshots(agent,world) }

Bindings injected into agent.properties only (not world).
```

---

## 8. Scheduler (`GdPAIPlanScheduler`)

```
submit_plan(agent, agent_bb, world_bb, actions, goals, max_depth, budget):
    cancel existing jobs for agent
    snap_agent = snapshot(agent_bb); snap_world = snapshot(world_bb)
    initial_provisions = extract(agent) + extract(world)

    action_specs = build(actions)  // registers callables → IDs
    goal_specs = build(goals)

    // Pre-filter satisfied goals
    satisfied = highest_reward goal where desired_state all true in initial state
    goal_specs = goal_specs.filter(not satisfied)
    satisfied_goal_index = goal_specs.empty ? satisfied : -1
    goal_specs.sort_by(reward desc)

    build provision_index + non_wildcard_actions
    ctx = SearchContext{...}
    engine = PlannerEngine(ctx, max_depth, cancel_flag)
        .heuristic(Dijkstra).termination(BestCost).budget(budget)
    spawn on Rayon pool

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

    4. Cleanup finished jobs (one frame grace)
```

---

## 9. Key Invariants

1. Preconditions hold *before* action runs (own effect never satisfies own preconditions).
2. Forward validation uses full world context (`Some(&branch.current_world)`).
3. Branch identity = goal + open needs + bindings + state (fingerprint hashes all).
4. One precondition per simulation step (re-queue after each).
5. Cost = sum(action_costs) always (recalculated, not accumulated).
6. Discovery caches per planning run (shared across branches).
7. Insertion position = earliest consumer (predecessor before first consumer).
8. Goal handling: see Scheduler step 5.
9. Bindings injected into agent only.
10. Greedy requirement clearing: one action clears ALL identical open requirements.
11. Stale pending cleanup via `request_map`.
12. Validity checks during discovery: builtin sync, custom async.
13. Custom preconditions evaluated during discovery (against initial-state sim) + tracked for verification.
14. Non-wildcard actions discovered once with empty bindings.

---

## 10. Termination

| Strategy | Behavior |
|----------|----------|
| `FirstComplete` | Return first valid plan |
| `BestCost` | Exhaust search (prune via `heuristic.prune_threshold_met`) |

Default: **BestCost** + Dijkstra.

---

*End of Algorithmic Pseudocode*
