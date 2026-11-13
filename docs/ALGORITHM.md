# Planning Algorithm

How the GdPlanningAI planner finds a plan, for readers who know GOAP basics. Sections 1-6 explain it; sections 7-9 are pseudocode of the engine for working on it; section 10 walks through an example.

The planner is the Rust crate in `addons/GdPlanningAI/rust/src/` (`planner/` and `scheduler.rs`). Users never touch it: they write `Action`, `Goal`, `Precondition`, `RequirementSpec` and `ProvisionSpec` in GDScript, the agent submits them through `GdPAIPlanScheduler`, and the plan comes back through `agent._on_plan_ready(dict)`.

______________________________________________________________________

## 1. Concept

GdPlanningAI is a backward-chaining GOAP planner: start from a goal, find actions whose effects satisfy it, then find actions that satisfy those actions' needs. It splits the work into two layers:

1. **Symbolic**: *requirements* ("what an action needs") are matched against *provisions* ("what an action offers"), like `fact("is_food", [])` or `binding("held_item", item_id)`. Backward chaining happens here, and it is fast because it is pattern matching.
1. **Simulation**: candidate chains run forward through the real GDScript callables for costs, effects and custom preconditions, on simulated blackboard snapshots. This is where a chain is accepted as a plan or discarded.

Symbolic matching alone is not enough: a binding can match while the cost or effect callable rules the action out, or an effect can break a later action's preconditions. So every backward-chained candidate is verified forward.

**Invariant:** preconditions hold *before* an action runs, so an action's own effect never satisfies its own preconditions. Verification re-checks each action's preconditions against the state just before it runs, which rejects a chain that only works because an effect patches its own precondition.

______________________________________________________________________

## 2. The two-phase search

A job's search (`step_search`) walks a priority queue of `PlanBranch`es. Each carries an action chain, its open needs, bindings, per-action costs, and the simulated agent and world snapshots. A need is attached to a *position*: the chain index of the action consuming it, so it must hold just before that action runs. Goal preconditions sit at `chain.len()`, the final state.

### Phase 1: backward discovery (`Searching`)

The expander (`find_candidates`) proposes actions that could satisfy a branch's open requirements and open preconditions:

- actions whose provisions match an open requirement, through a `provision_index` from `(ProvisionKind, name)` to actions; a `Fact` requirement also pulls in actions providing the matching `FactWildcard`;
- every **non-wildcard** action, since its effect may satisfy an open precondition that no provision names.

Each candidate is first filtered against the **initial** state: its `validity_checks` must pass (builtin checks synchronously, custom ones through a callback). A wildcard action is discovered once per matching requirement x provision pair, with the match as its bindings; a non-wildcard action once with empty bindings. Discovery simulates the candidate from the initial state, and a candidate must satisfy at least one open need to be emitted.

Each emitted candidate is inserted *before its earliest consumer* (last, when it only serves the goal). It clears every open requirement identical to the one it satisfied, adds its own needs (minus those already open or already true in the initial state), and the new branch is enqueued. When no open needs remain, the branch flips to `Verifying`.

### Phase 2: forward verification (`Verifying`)

`process_simulation` replays the chain from the initial snapshots, one step per queue pop:

1. On entering verification, each action's custom preconditions are re-opened at its position and the goal's preconditions at the end of the chain, since a later insertion can undo a need an earlier action satisfied.
1. The current action's builtin preconditions are re-checked against the simulated state; a failure invalidates the branch.
1. One open precondition at this position is evaluated per step, and the node is re-queued, so custom-precondition callbacks are serviced between steps.
1. The action's requirements are checked against the current state (`requirement_holds_in_state`, section 3).
1. The action is simulated: its cost is the one recorded at discovery, and its effect is applied (section 4). Effects are deterministic and branches share snapshots, so a step already simulated from the same state with the same action and bindings is reused (the first step reuses its discovery result), and branches reaching it together wait on one request.
1. At the end of the chain every need has been checked, the goal's last, and the branch is recorded as the best plan if it is the cheapest so far.

______________________________________________________________________

## 3. Requirements and provisions

An `Action` declares **requirements** (the causal links it depends on) and **provisions** (what it offers later actions). All backward chaining runs through these.

| Requirement                  | Meaning                                                                            |
| ---------------------------- | ---------------------------------------------------------------------------------- |
| `BindingExists(name)`        | Some earlier action must have bound `name` (or the agent property already exists). |
| `BindingEquals(name, value)` | `name` must be bound to `value` (or the agent property already equals it).         |
| `BindingInSet(name, set)`    | `name` must resolve to an object in the world-object group `set`.                  |
| `Fact(name, args)`           | A named world fact must hold, optionally with concrete arguments.                  |

| Provision              | Meaning                                                   |
| ---------------------- | --------------------------------------------------------- |
| `Binding(name, value)` | Binds `name` to `value` for consumers later in the chain. |
| `Fact(name, args)`     | Provides a concrete fact, e.g. `fact("is_food", [])`.     |
| `FactWildcard(name)`   | Provides *any* fact with that name, whatever its args.    |

**Matching** (`provision_satisfies_requirement`) is structural:

| Provision         | Requirement            | Matches when                                         |
| ----------------- | ---------------------- | ---------------------------------------------------- |
| `Binding(n, v)`   | `BindingExists(n)`     | names equal                                          |
| `Binding(n, v)`   | `BindingEquals(n, v2)` | names and values equal                               |
| `Binding(n, obj)` | `BindingInSet(n, set)` | names equal and `obj` is in world group `set`        |
| `Fact(n, args)`   | `Fact(n, args2)`       | names equal and args equal (or the consumer's empty) |
| `FactWildcard(n)` | `Fact(n, _)`           | names equal, any args                                |

`FactWildcard` is how `GoToAction` works: it provides `fact_wildcard("at_target")`, and interaction actions require `fact("at_target", [location])`. Matching the wildcard binds the consumer's arguments, so one GoTo becomes a candidate per target, made concrete with its binding during verification so it only satisfies `at_target` for its own target. The bindings return with the plan, which is how the agent knows where to go.

**Holding in state** (`requirement_holds_in_state`, during verification):

| Requirement              | Holds when                                                                       |
| ------------------------ | -------------------------------------------------------------------------------- |
| `BindingExists(n)`       | `n` is bound, or the agent property `n` is neither null nor an empty string      |
| `BindingEquals(n, v)`    | `n` is bound to `v`, or the agent property `n` equals `v`                        |
| `BindingInSet(n, set)`   | `n`'s first bound value (or the agent property) is a world object in group `set` |
| `Fact("at_target", [t])` | the agent's location equals target `t`'s in the world (falling back to bindings) |
| any other `Fact`         | assumed true (preconditions cover it)                                            |

Bindings are injected into the **agent** blackboard only, never the world's, using the first value of each binding.

______________________________________________________________________

## 4. The simulation layer

**Preconditions.** A *builtin* precondition is a comparison over the agent or world blackboard, e.g. `agent_property_less_than("hunger", 15)`, evaluated synchronously in the engine. A *custom* one wraps a GDScript callable that must run on the main thread. Custom preconditions run during discovery (against the state the action would produce from the initial state, cached per `(action, precondition, bindings)`) and again during verification (against the simulated state, not cached). A goal's desired state is a list of preconditions; the unmet ones become the root branch's open preconditions.

**Costs.** `get_action_cost()` returns a float, evaluated against the **initial** state during discovery; that cost seeds the branch, so a plan's cost is the sum of initial-state costs. Lookups go: the branch's recorded cost, a `GetCost` response in hand, the per-run discovery cache, and only then a fresh `GetCost` request. Since every inserted action carries its discovery cost, verification in practice never calls the cost callable again. An action with no cost callable costs 1.0. An int or float is accepted; anything else coerces to `INFINITY`, which makes the action unusable: it never becomes a candidate, and an infinite cost in verification invalidates the branch. Actions gate themselves this way: `AddFuelAction` returns `INFINITY` when the campfire is full.

**Effects.** An action declares its effect as data in `get_effect_spec()`, which the engine applies to the snapshots natively, with no callback. Only an action without a usable spec falls back to `simulate_effect()` through an `ApplyEffect` callback, and those fallbacks are capped per job by `max_effect_fallbacks` (default 128): past the cap the branch is dropped and one warning is logged. An action with neither simulates as the identity. Effects only touch simulated copies; the live scene is never touched while planning. Effects may be *optimistic*: an action can report a plausible post-state before every binding is concrete (the examples' `optimistic_unbound_restore` fallback), and the real value resolves when the plan runs. This lets GoTo chain in front of an interaction before its target is known.

______________________________________________________________________

## 5. Search control

- **Priority.** The queue is a binary heap ordered by branch cost (Dijkstra / uniform-cost). A branch's cost is always recomputed as the sum of its `action_costs`, so reordering the chain never corrupts it.
- **Termination.** `BestCost` (the default) keeps searching after a plan is found, pruning branches that cannot beat it and keeping only strictly cheaper plans, so the result is cost-optimal under the declared costs. `FirstComplete` returns the first verified plan, which is faster but not necessarily cheapest.
- **Iteration budget.** One engine step pops at most `iteration_budget` nodes (default 20000, at least 100), then re-enqueues the node and returns `Pending(0)`: a sentinel the scheduler resumes unconditionally next frame.
- **Time slice.** `time_slice_ms` (default 10, 0 disables) bounds a step's wall-clock time, checked every 128 pops, yielding `Pending(0)` the same way. It changes only how the work spreads across frames, never the result.
- **Depth and cycles.** `max_depth` (`max_recursion` in GDScript; at least 1) drops a branch whose chain reaches it and prunes one whose open requirements outnumber it. The second guard stops mutually referencing actions from growing a branch forever.
- **Dedup.** A fingerprint of goal, open needs, bindings and state skips a branch already visited at equal or lower cost.

The search is sound but bounded: depth, budget and the cycle guard trade completeness for frame-time guarantees.

______________________________________________________________________

## 6. Async architecture and goals

**Threads.** The engine runs on a Rayon worker; GDScript callables must run on the Godot main thread. Godot values are not thread-safe, so the worker only sees `BlackboardSnapshot`s, converted on the main thread both ways: a `VariantSnapshot` stores primitives directly, live objects as instance-id handles (`null` once freed) and everything else as `var_to_bytes` blobs. The scheduler registers each `Callable` in a per-job `callable_registry`, and the engine's specs carry only the ids. The engine is moved, never shared: a worker step takes it and sends it back with its result.

**Callbacks.** When the engine needs a callable, it sends a `CallbackRequest` and returns `Pending(id)`, parking the node. Each frame the main thread dispatches pending requests and answers on the job's channel (section 9). A parked job resumes only when it can progress: always for the `Pending(0)` sentinel, otherwise when a response for it was dispatched this frame or is already in `completed_request_ids`. A finished job is reaped a frame later, and its engine is dropped on a background thread, since freeing a large search can take up to about 1.5 s.

**Cancellation.** Each job has a cancel flag, set on re-submission and by `cancel_agent_jobs`, `clear_active_jobs` and `cancel_all_jobs`, and checked at three points in `step_search`. A cancelled job returns `Complete(None)` without a plan; its engine is still recovered so the debug tree can be read.

**Goals.** `submit_plan` receives every goal the agent has. Goals already satisfied in the initial state are dropped, remembering the highest-reward one; the rest are searched in descending reward order, and a goal with no plan hands over to the next. If every goal was satisfied, the result is an empty success plan for the remembered goal. The first goal with a plan ends the run, so the agent pursues the highest-reward goal it can reach.

______________________________________________________________________

## 7. Engine pseudocode: types and the search loop

Conceptual types (see the source for exact ones):

```
ActionSpec     { name, cost_fn?, effect_spec?, effect_fn?, preconditions[], validity_checks[], requirements[], provisions[], dependent_objects[] }
GoalSpec       { name, reward, desired_state[PreconditionSpec], original_index }
Precondition   { Builtin(target, op, property, value?) | Custom(callable_id, deps[]) }
PlanBranch     { action_chain[], action_costs[], action_bindings[(pos,name,values)], open_preconditions[(pos,spec)],
                 open_requirements[(pos,spec)], state: Searching|Verifying, simulation_index, current_agent, current_world,
                 cost, tree_node_id, goal_index }
               // action_costs is seeded from the discovery cost on insertion; cost = sum(action_costs)
SearchNode     { branch, resumed, callback_response, expanded_candidates[(action_idx,bindings)] }
SearchContext  { actions[], initial_agent, initial_world, initial_provisions[], request_tx, engine_response_tx,
                 caches..., provision_index[(kind,name) -> action_idxs], non_wildcard_actions[] }
```

Per-run caches, shared across branches: `discovery_results[(action,bindings)] -> {agent,world,cost}`, `discovery_costs[(action,bindings)]`, `discovery_precond_results[(action,spec,bindings)]`, their pending maps (stale entries cleaned through `discovery_request_map`), and on the engine for verification `step_cache[(pre_agent,pre_world,action,bindings)] -> post-state` with `step_pending` for in-flight effects.

```
function step_search(goals):
    if cancel_flag: return Complete(None)

    // resume callbacks
    while response = response_rx.try_recv():
        if cancel_flag: parked_nodes.clear(); return Complete(None)
        discovery response: write caches, clear pending; step response: step_cache[key] = snapshots
        for parked node waiting on it: node.resumed = true; node.callback_response = response; enqueue(node)

    iterations = 0
    while node = queue.pop():
        iterations += 1
        if BestCost and node.priority >= best_cost: re-enqueue; break
        if iterations > budget: enqueue(node); return Pending(0)
        if cancel_flag: return Complete(None)
        if not resumed and state == Searching:
            if visited[fingerprint(branch)] <= branch.cost: continue
            visited[fingerprint(branch)] = branch.cost
        resumed = false          // a resumed node keeps expanded_candidates, so it skips what it already added
        if branch.cost >= best_cost: continue
        if time_slice_ms > 0 and iterations % 128 == 0 and elapsed >= time_slice: enqueue(node); return Pending(0)

        Searching:
            if no open needs: begin_verification(branch, goal); enqueue; continue
            if action_chain.len >= max_depth: continue
            candidates = find_candidates(branch, ctx)
            for cand in candidates.ready, skipping (action,bindings) already in expanded_candidates:
                new = branch.clone()
                pos = earliest consumer of cand's satisfied needs (chain end if it only serves the goal)
                insert cand.action and its discovery cost at pos; shift later positions by 1
                for each satisfied requirement: remove ALL identical open requirements;
                    record the provider's binding at pos and the consumer bindings at each removed position
                remove the satisfied open preconditions
                add cand's own needs at pos, skipping ones already open, builtin preconditions true initially,
                    and requirements met by initial_provisions
                new.recalculate_cost()
                if no open needs: begin_verification(new, goal)
                if new.open_requirements.len > max_depth: prune; continue      // cycle guard
                enqueue(new)
            if candidates.pending_id: park node

        Verifying:
            match process_simulation(node):
                Ready -> enqueue; Pending(id) -> park; Invalid -> discard
                Complete -> if FirstComplete and best_plan (unless it is empty and goals remain): return Complete(best_plan)
                            else keep searching

    if parked_nodes: return Pending(any parked id)
    if more goals and (no best_plan or best_plan is empty): reset best; advance to next goal; return Pending(0)
    return Complete(best_plan or a failed PlanResult)          // Complete(None) means cancelled
```

______________________________________________________________________

## 8. Engine pseudocode: discovery and verification

```
function find_candidates(branch, ctx) -> {ready[], pending_id?}:
    actions = BTreeSet()
    for (_, req) in open_requirements:
        actions += provision_index[kind(req), req.name]
        if req is Fact: actions += provision_index[FactWildcard, req.name]
    actions += non_wildcard_actions

    for idx in actions:
        for check in actions[idx].validity_checks:              // against the initial state
            Builtin: if not evaluate(initial_agent, initial_world): skip action
            Custom: cached result -> skip if false; pending or newly requested -> mark pending, skip action
        if action has a FactWildcard provision:
            for (req_idx, req) in open_requirements, prov in provisions where provision_satisfies_requirement(prov, req):
                bindings = binding_values(prov, req)             // FactWildcard vs Fact(n,args) -> [(n, args)]
                res = discovery(idx, bindings)                   // result cache -> pending map -> cost, then effect
                if res pending: mark pending; continue
                pre = open preconditions true in res (custom: cache, else request and stop checking)
                if no custom check is still pending: emit {idx, [(req_idx,req,prov)], pre, bindings}
        else:
            reqs = each open requirement matched by some provision (once per requirement)
            res = discovery(idx, [])
            if res pending: mark pending; continue
            pre = open preconditions true in res (custom: cache, else request and stop checking)
            if no custom check is still pending and (reqs or pre): emit {idx, reqs, pre, []}
    return {ready, pending_id if any pending}

function begin_verification(branch, goal):
    for (pos, action) in action_chain: open_preconditions += (pos, each custom precondition of action)
    open_preconditions += (action_chain.len, each goal precondition)
    state = Verifying; simulation_index = 0; current_agent, current_world = initial

function process_simulation(node) -> Ready | Pending(id) | Invalid | Complete:
    bindings = branch.bindings_at(simulation_index)
    if simulation_index < action_chain.len:
        for builtin pre of action: false -> Invalid
    if one open precondition at simulation_index: evaluate it (custom: response or request -> Pending);
        false -> Invalid; true -> remove it, return Ready
    if simulation_index < action_chain.len:
        for req of action: if not requirement_holds_in_state(req, current_agent, current_world, bindings): Invalid
        key = (current_agent, current_world, action, bindings)        // snapshots compared by identity
        if cost known and step_cache[key]: take it
        elif simulation_index == 0 and discovery holds this step: take the discovery result
        elif cost known and step_pending[key] = id and no response: return Pending(id)   // share the request
        else: simulate(action) -> cost (section 4), INFINITY -> Invalid; effect_spec natively, else the response
              or an ApplyEffect request (within max_effect_fallbacks, else Invalid)
        advance current_agent, current_world; simulation_index += 1; recalculate_cost(); return Ready
    if branch.cost < best_cost: best_cost = branch.cost; best_plan = branch
    return Complete
```

______________________________________________________________________

## 9. Callbacks and the scheduler

```
Engine -> main:  CallbackRequest { request_id, callable_id, kind, bindings[], response_tx }
                 kind: GetCost(agent,world) | ApplyEffect(agent,world) | EvalCustomPrecond(agent,world)
Main -> engine:  PlannerCallback { request_id, Float(f64) | Bool(bool) | UpdatedSnapshots(agent,world) }

dispatch_callback (main thread): rebuild blackboards, inject bindings (agent only, values[0]), call the callable;
    GetCost -> f64 (non-numeric -> INFINITY); ApplyEffect -> simulate_effect, re-snapshot; EvalCustomPrecond -> bool (non-bool -> false)

submit_plan(agent, agent_bb, world_bb, actions, goals, max_recursion, iteration_budget, time_slice_ms, max_effect_fallbacks):
    cancel the agent's existing jobs
    snapshot both blackboards; initial_provisions = from agent + world
    build action and goal specs (registering callables -> ids)
    drop goals already satisfied (builtin conditions on the snapshots, custom ones on the live blackboards),
        remembering the highest-reward one; sort the rest by reward desc
    PlannerEngine::new(inputs, config)                     // termination BestCost by default
    run the first step on the Rayon pool

process_callbacks()  // each frame
    1. recover engines from workers: Complete(plan) -> empty plan and a satisfied goal? empty success for it;
       agent._on_plan_ready(plan). Pending(id) -> job.pending_request_id = id
    2. dispatch Godot callbacks, sending each response
    3. resume jobs that can progress: pending id 0, a response this frame, or one already received
    4. reap finished jobs a frame later, dropping their engines on a background thread
```

**Logging.** One process-wide level (`plugin.cfg` `log_level`, default 2 = Info, set by the autoload; `GdPAIPlanScheduler.set_log_level` changes it at runtime, up to 3 = Debug). Info prints the `submit_plan` summary, `Plan complete`, one `RESULT goal='…' success=… actions=… cost=… Branches: … | Time: …ms` line per searched run, and warnings (among them "Search budget exceeded" on every budget yield). Debug adds search transitions (`Pending` / `Resuming`) and one `NOT resuming` notice per stall, and records the full search tree, fetched with `get_debug_tree(agent)` and never printed; its dump marks failed verification steps (requirements and preconditions) as `FWD [FAIL]`. A branch counter always runs so `RESULT` reports search size; time-slice yields are not logged.

______________________________________________________________________

## 10. Worked example: GoTo -> PickUp -> EatHeldFood

In the campfire scene this chain satisfies hunger. Suppose the agent is hungry (hunger = 60) and nowhere near a banana.

| Action              | Validity              | Preconditions                        | Requirements                                         | Provisions                                                          |
| ------------------- | --------------------- | ------------------------------------ | ---------------------------------------------------- | ------------------------------------------------------------------- |
| `EatHeldFoodAction` | agent has `hunger`    | hunger > 0; custom "is holding food" | `binding_exists("held_item")`; `fact("is_food", [])` | none                                                                |
| `PickupAction`      | object valid          | `held_item == ""`                    | `fact("at_target", [banana_location])`               | `binding("held_item", item_id)`; `fact("is_food", [])` (Food group) |
| `GoToAction`        | entity, location data | none                                 | none                                                 | `fact_wildcard("at_target")`                                        |

`HungerGoal` has reward `max(0, hunger)` and wants hunger below `max(sated_hunger, current - hunger_drop)` (both 15.0 by default): here `agent_property_less_than("hunger", 45)`.

**Backward.** The goal's precondition is open at position 0 (the end of the empty chain). `EatHeldFoodAction` comes in through the non-wildcard set: it passes validity and its simulation lowers hunger. It adds `binding_exists("held_item")`, `fact("is_food", [])` and its custom "is holding food" precondition (custom preconditions are never assumed true initially). `PickupAction` provides both requirements, and its simulated effect satisfies the custom check; its `held_item == ""` precondition already holds initially, so only `fact("at_target", [banana_location])` is added. Being a `Fact`, it pulls in the `FactWildcard` provider `GoToAction`, bound to the banana's location and inserted first. GoTo needs nothing, so the chain is complete:

```
goal: hunger < 45
  <- EatHeldFoodAction  (needs held_item bound, is_food)
    <- PickupAction     (needs at_target [banana_location])
      <- GoToAction     (needs nothing)
```

**Forward.** Verification replays the chain from the initial state: GoTo moves the agent to its bound target, which satisfies Pickup's `at_target`; Pickup binds `held_item` and provides `is_food`; EatHeldFood's custom "is holding food" check passes and its effect lowers hunger and clears `held_item`. The goal holds at the end, so the branch becomes the best plan. BestCost keeps looking for something strictly cheaper, such as the tree-shake route (`ShakeTreeAction` costs 10.0), then delivers the cheapest to `agent._on_plan_ready(dict)`.

The scene's other chains work the same way: `GoTo -> Pick Up Wood -> Add Fuel`, where `AddFuelAction` requires `binding_equals("held_item", "wood")`, which `PickUpWoodAction` provides; and cook-and-eat, where `CookPotatoAction` costs `INFINITY` while the fire is low, which couples hunger to keeping the fire going.
