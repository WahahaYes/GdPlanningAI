# Planning Algorithm

A readable explanation of how the GdPlanningAI planner works, aimed at a developer who already understands GOAP basics. For the exhaustive step-by-step pseudocode, see [PLANNER_PSEUDOCODE.md](PLANNER_PSEUDOCODE.md). That document is the terse algorithmic reference; this one is the intuition behind it.

The planner lives in the Rust crate at `addons/GdPlanningAI/rust/src/`. Users never touch that code directly. They define `Action`, `Goal`, `Precondition`, `RequirementSpec`, and `ProvisionSpec` in GDScript, submit them through the `GdPAIPlanScheduler`, and receive a `Plan` back through `agent._on_plan_ready(dict)`.

______________________________________________________________________

## 1. High-level concept

GdPlanningAI is a GOAP planner with a twist. Classic GOAP works by backward chaining: start from a goal, find actions whose effects would satisfy it, then recursively find actions to satisfy those actions' preconditions. This planner does the same, but it separates that reasoning into two cooperating layers:

1. **A symbolic layer** that matches *requirements* against *provisions*. These are lightweight, structured descriptions of "what an action needs" and "what an action provides", like `fact("is_food", [])` or `binding("held_item", item_id)`. Backward chaining happens here, which is fast because it is pure pattern matching.
1. **A simulation layer** that runs the candidate chains forward, executing real GDScript callables for costs, effects, and custom preconditions against simulated blackboard snapshots. This is where a candidate chain is accepted or rejected as a real, valid plan.

Symbolic matching alone is not enough. A binding might match textually while `get_action_cost()` or `apply_effects()` still fails at runtime, or the simulated state violates an action's own preconditions once effects apply. So every backward-chained candidate is later *verified* by simulating it forward.

```
      backward chaining (cheap, symbolic: requirements <-> provisions)
      ^                                                         |
      | candidate chain, discovered in reverse                 |
  goal needs                                               provisions
      |                                                         |
      +----------- forward simulation (GDScript callables) ----+
                   run preconditions, costs, effects in order
                   -> valid plan or discard
```

### A key invariant

Preconditions hold **before** an action runs. An action's own effect never satisfies its own preconditions. During verification the engine deliberately re-checks each action's builtin preconditions against the state *as it is just before* that action executes, so a chain that only "works" because an effect patches its own precondition is rejected.

______________________________________________________________________

## 2. The two-phase search

Every planning job runs two interleaved phases. The search state machine (`step_search`) walks a priority queue of `PlanBranch` values, each carrying an action chain, its open requirements and preconditions, bindings, accumulated cost, and snapshots of the simulated agent and world state.

### Phase 1: backward candidate discovery

A branch in the `Searching` state has a list of *open needs*: `open_requirements` (position + requirement) and `open_preconditions` (position + precondition). Position 0 means "before the whole chain". The planner asks the *expander* (`find_candidates`) which actions could satisfy those needs:

- Actions whose provisions match an open requirement, found via a `provision_index` that maps `(ProvisionKind, name)` to action indices. A `Fact` requirement also pulls in actions that provide the matching `FactWildcard`.
- All **non-wildcard** actions, because their effects may satisfy an open precondition even though their provisions are too specific to match a requirement by name.

Each candidate is filtered against the **initial** state first: `validity_checks` must pass (builtin checks synchronous, custom checks async through the callback channel). Wildcard actions are explored once per requirement x provision pair; non-wildcard actions once with empty bindings. A candidate must satisfy at least one requirement or precondition to be emitted.

For each ready candidate the planner computes an insertion position (the earliest consumer, so the action is placed *before* its first consumer), inserts it into the chain, greedily clears every open requirement with a spec identical to the one satisfied, filters the new action's own needs (dropping anything already open or satisfied by the initial state), and re-enqueues the branch.

When the chain has no open needs left, the branch flips to `Verifying`, resets its simulation index to 0, restores the initial snapshots, and is re-enqueued.

### Phase 2: forward verification

A branch in the `Verifying` state is run through `process_simulation`. This executes the chain from first to last action against simulated snapshots, one careful step at a time:

1. On the first step, requirements at position 0 (already satisfied by initial provisions) are cleared.
1. The current action's own builtin preconditions are re-checked against the *current simulated* state. In Verifying, a failure here is fatal: `Invalid`.
1. Open preconditions at this position are evaluated one at a time. Each satisfied precondition is removed and the node is re-queued, so callbacks for custom preconditions can be serviced atomically between steps.
1. The action's requirements are validated against the current state through `requirement_holds`, which resolves them against the recorded bindings.
1. The action is simulated: cost comes from a three-tier cache (the recorded branch cost, a callback response, or a fresh `GetCost` call), and the effect comes from an `ApplyEffect` callback or the identity function if the action declares no effect. Snapshots advance, wildcards are concretized via `clear_requirements_from_provisions`, and `sim_index` increments.
1. At the end of the chain, any remaining open needs make the branch `Invalid`. Otherwise the branch cost is compared against the best cost seen so far and recorded as `best_plan` if cheaper.

A sketch of the whole flow:

```
   Searching (backward)              Verifying (forward)
   --------------------              --------------------
   open needs remain?   -- no -->    process_simulation(), one step per loop:
        |  yes                          - clear pos-0 requirements (step 0)
        v                               - re-check own builtin preconditions
   find_candidates()                    - satisfy one open precondition
        |  builtin: sync                - validate requirements with bindings
        |  custom: CallbackRequest      - cost (3-tier) + effect (ApplyEffect)
        |           -> Pending(id)      - advance snapshots, sim_index++
        v                               - end of chain: best cost -> best_plan
   insert action, clear needs
   filter own new needs; cycle guard
        |
        +--> if no open needs: Verifying (reset to initial, sim_index = 0)
```

______________________________________________________________________

## 3. The symbolic layer: requirements and provisions

An `Action` declares two matching surfaces. **Requirements** are the causal links it depends on; **provisions** are what it offers to earlier actions. The provision index and the expander do all backward chaining through these.

### Requirement kinds

| Kind | Meaning | |------|---------| | `BindingExists(name)` | Some earlier action must have bound `name` (or the agent property already exists). | | `BindingEquals(name, value)` | `name` must be bound to `value` (or the agent property already equals it). | | `BindingInSet(name, set)` | `name` must resolve to an object that is a member of the world-object group `set`. | | `Fact(name, args)` | A named world fact must hold, optionally with concrete arguments. |

### Provision kinds

| Kind | Meaning | |------|---------| | `Binding(name, value)` | Binds `name` to `value` for consumers later in the chain. | | `Fact(name, args)` | Provides a concrete fact, e.g. `fact("is_food", [])`. | | `FactWildcard(name)` | Provides *any* fact with that name, whatever its arguments. |

`FactWildcard` is the mechanism behind `GoToAction`: it provides `fact_wildcard("at_target")` and every interaction action requires `fact("at_target", [some_location])`. The wildcard binds the consumer's concrete arguments during discovery, so one generic navigation action chains in front of any location-dependent interaction.

### Matching rules

Matching is structural, performed by `provision_satisfies`:

| Provision | Requirement | Match condition | |-----------|-------------|-----------------| | `Binding(n, v)` | `BindingExists(n)` | names equal | | `Binding(n, v)` | `BindingEquals(n, v2)` | names and values equal | | `Binding(n, obj)` | `BindingInSet(n, set)` | name matches and `obj` is in world group `set` | | `Fact(n, args)` | `Fact(n, args2)` | names match and args are equal (or consumer args empty) | | `FactWildcard(n)` | `Fact(n, _)` | names match, any arguments |

During verification, `requirement_holds` re-checks each requirement against the current simulated state, resolving `Binding*` requirements against recorded bindings or live agent properties, and `Fact` requirements against the world context (e.g. `at_target` compares agent and target locations).

Bindings are injected into the **agent** blackboard only, never the world blackboard, using the first value of each binding vector.

______________________________________________________________________

## 4. The simulation layer

The symbolic layer decides *which* chains to try; the simulation layer decides which chains *actually work*.

### Preconditions: builtin vs custom

`Precondition` comes in two forms. **Builtin** preconditions describe a simple comparison over the agent or world blackboards, e.g. `agent_property_less_than("hunger", 15)`, and evaluate synchronously inside the engine. **Custom** preconditions wrap a GDScript callable that must run on the main thread; they are fired through the callback channel during discovery and again during verification, with results cached per `(action, precondition, bindings)` so repeated branches do not re-run them.

The goal's desired state is itself a list of preconditions. Unmet ones become open preconditions at position 0.

### Cost callables

`get_action_cost()` returns a `float`. Costs are fetched through a three-tier cache: the cost already recorded on the branch, then a `GetCost` response in hand, then the shared per-run discovery cache. Only if all three miss does the engine fire a fresh `GetCost` request and return `Pending(id)`.

A non-numeric return value coerces to `INFINITY`, marking the action invalid. This is how actions gate themselves at runtime: `AddFuelAction` returns `INFINITY` when the campfire is full, and `CookPotatoAction` when fuel is below its minimum. Infinite cost does not just make an action expensive, it makes any branch through it uncompetitive.

### Effect callables

`apply_effects()` is called via `ApplyEffect`, which returns updated agent and world snapshots. Actions without an effect callable simulate as the identity function. Effects mutate *simulated copies* only; the live scene graph is never touched during planning.

### Optimistic simulation

Simulation does not need every binding concrete. An action may restore hunger even when the exact food value is unknown; the example scenes use an `optimistic_unbound_restore` fallback. The action reports a plausible post-state during discovery and verification, and the real value resolves when the plan is performed. This lets the planner chain `GoToAction` in front of an interaction action before the exact target binding is resolved.

______________________________________________________________________

## 5. Search prioritization

### Dijkstra / uniform-cost

The queue is a binary heap ordered by branch cost, and the heuristic is Dijkstra (uniform-cost). A branch's cost is always the sum of its `action_costs`, recomputed rather than accumulated, so reordering the chain never corrupts it.

### Termination strategies

Two `TerminationStrategy` values control when planning stops:

| Strategy | Behavior | |----------|----------| | `FirstComplete` | Returns the first valid plan the moment verification completes. | | `BestCost` | Keeps searching, pruning branches whose priority is no better than the best cost found, and records the strictly cheaper plan at each completion. |

The default is **BestCost with Dijkstra**. Because BestCost keeps only strictly cheaper plans and prunes at the best-cost threshold, the returned plan is cost-optimal under the declared cost functions. `FirstComplete` trades that guarantee for speed: it returns the first valid plan, which is not necessarily the cheapest.

### Iteration budget

Each run has an `iteration_budget` (default 20000, clamped to at least 100). When the loop exceeds it, the current node is re-enqueued and `step_search` returns `Pending(0)`. The `0` is a sentinel the scheduler resumes unconditionally on the next frame, no callback response needed. A single unbounded search becomes a resumable, frame-budgeted one.

### max_depth and the cycle guard

`max_depth` (called `max_recursion` in the GDScript-facing API; 4 in the hunger config, 6 in the campfire config) is clamped to at least 1 and serves two purposes. A branch whose `action_chain` reaches `max_depth` is dropped, and a branch whose `open_requirements` count exceeds `max_depth` is pruned. The second guard stops the backward search from looping forever on mutually referencing actions, where every new action adds a requirement without closing one.

Branches are also deduplicated by a fingerprint of goal, open needs, bindings, and state: a branch already visited at equal or lower cost is skipped.

______________________________________________________________________

## 6. Async architecture

The planner runs on a Rayon worker thread while GDScript callables must run on the Godot main thread. The two sides communicate through a per-job channel pair.

### The callback protocol

When the planner needs a callable evaluated, it sends a `CallbackRequest` on the job's `request_tx` and returns `Pending(id)` from its step, parking the current node. The request carries a `request_id`, the registered `callable_id`, a `kind`, and the current bindings:

```
Planner (Rayon worker)              Main thread (per frame)
----------------------              -----------------------
CallbackRequest { request_id,       process_callbacks():
  callable_id, kind: GetCost |        look up callable in registry
  ApplyEffect | EvalCustomPrecond,    rebuild blackboards, inject bindings
  bindings, response_tx }             call cost/effect/eval callable
  -------------------------------->  PlannerCallback { request_id,
                                     Float | Bool | UpdatedSnapshots }
```

Dispatch semantics (`dispatch_callback`):

- `GetCost`: rebuild blackboards, inject bindings, call `cost_callable(agent, world)`, coerce to `f64` (non-numeric -> `INFINITY`, meaning invalid).
- `ApplyEffect`: same setup, call `effect_callable`, re-snapshot into `UpdatedSnapshots`.
- `EvalCustomPrecond`: call `eval_callable`, coerce to `bool` (non-bool -> `false`).

Bindings are injected into the agent blackboard only, using `values[0]`.

### Pending and resume semantics

A callback response either completes a pending discovery request (cache write) or resumes a parked node (stored as its `callback_response` and re-enqueued). Resume gating is precise:

- A job whose `pending_request_id == 0` (the budget-yield sentinel) is always resumable.
- Otherwise it resumes only if its response was dispatched this frame or is already recorded in `completed_request_ids`.

A parked node never resumes before its answer exists, and budget yields never stall waiting for a response that will never come.

______________________________________________________________________

## 7. Multi-goal handling

`submit_plan(agent, agent_bb, world_bb, actions, goals, max_recursion, iteration_budget)` receives every goal an agent owns. The scheduler reduces the set before searching:

1. Any goal whose desired state already holds in the initial state is satisfied. The **highest-reward** satisfied goal is remembered as `satisfied_goal_index`; all satisfied goals are dropped.
1. The remaining unsatisfied goals are sorted by reward descending, so the most urgent need is searched first.
1. If every goal was satisfied, the planner emits an empty success plan for the remembered goal. If any goal is unsatisfied, it searches only the unsatisfied set.

Within a run, when the current goal's search completes with an empty best plan and goals remain, the engine advances to the next goal, so the planner pursues the highest-reward goal it can actually achieve.

______________________________________________________________________

## 8. Cancellation

Each job owns an `Arc<AtomicBool>` cancel flag. It is set when the job is re-submitted, by `cancel_agent_jobs`, `clear_active_jobs`, or `cancel_all_jobs`. The flag is checked at three points inside `step_search`, so a stuck or superseded job stops promptly. A cancelled job returns `Complete(None)` without delivering a plan, but the engine is still recovered so the debug tree can be inspected on the main thread.

______________________________________________________________________

## 9. Worked example: GoTo -> PickUp -> EatHeldFood

The campfire scene uses this exact chain to satisfy the hunger goal. This is trace A from the examples research. Suppose the agent is hungry (hunger = 60), standing nowhere near a banana.

The actions involved, as declared by the author:

| Action | Validity | Preconditions | Requirements | Provisions | |--------|----------|---------------|--------------|------------| | `EatHeldFoodAction` | agent has `hunger` | hunger > 0; custom "is holding food" | `binding_exists("held_item")`; `fact("is_food", [])` | none | | `PickupAction` | object valid | `held_item == ""` | `fact("at_target", [banana_location])` | `binding("held_item", item_id)`; `fact("is_food", [])` (banana is in the Food group) | | `GoToAction` | entity, location data | none | none | `fact_wildcard("at_target")` |

The `HungerGoal` has reward `max(0, hunger)` and a desired state that hunger be lower than `max(0, current - 15)`, say `agent_property_less_than("hunger", 45)`.

**Backward phase.** The engine initializes a branch with the initial snapshots. The desired state is not yet true, so `agent_property_less_than("hunger", 45)` becomes an open precondition at position 0. The expander finds `EatHeldFoodAction` through the non-wildcard set: its validity checks pass and its simulation shows hunger dropping, so it is a candidate. Its own needs are added at position 0: `binding_exists("held_item")` and `fact("is_food", [])`.

`binding_exists("held_item")` is satisfied by `PickupAction`, which provides `binding("held_item", item_id)` and `fact("is_food", [])` for food-group objects; that same provision clears the `fact("is_food", [])` requirement. `PickupAction` brings its own needs: the precondition `held_item == ""` (true in the initial state, so dropped) and the requirement `fact("at_target", [banana_location])`.

Because `at_target` is a `Fact` requirement, the expander also consults the `FactWildcard` branch of the provision index. `GoToAction` provides `fact_wildcard("at_target")`, binds the consumer's concrete arguments, and is inserted before `PickupAction`. It has no needs of its own, so the chain is complete.

```
goal: hunger < 45
  ^ requires: EatHeldFoodAction (needs binding_exists held_item + fact is_food)
    ^ requires: PickupAction (needs fact at_target [banana_location])
      ^ requires: GoToAction (needs nothing)
```

**Forward verification.** The branch now flips to `Verifying`. `sim_index` is reset to 0 and the initial snapshots are restored. `process_simulation` walks the chain:

1. At `GoToAction`, the at_target requirement from earlier positions is cleared against initial provisions. The action simulates as identity (no cost or effect callable in this trace), and the simulated agent moves to the banana.
1. At `PickupAction`, `at_target` holds and the `held_item == ""` precondition holds. The effect binds `held_item = item_id` and records `is_food`. Cost 1.0.
1. At `EatHeldFoodAction`, the custom "is holding food" precondition passes (held_item is bound), and `binding_exists("held_item")` plus `fact("is_food", [])` hold. The effect reduces hunger and clears `held_item`. Cost `eat_duration`.
1. No open needs remain, so the branch's total cost is recorded as `best_plan`.

Under BestCost + Dijkstra, the engine keeps searching for a strictly cheaper plan. If a banana is reachable, the direct GoTo -> PickUp -> Eat chain wins over alternatives such as the tree-shake route, whose `ShakeTreeAction` costs 10.0 versus the banana chain's small pickup cost. The final plan is delivered to `agent._on_plan_ready(dict)`.

Two other chains from the same scene use the identical machinery: the fire chain `GoTo -> Pick Up Wood -> Add Fuel` (trace C), where `AddFuelAction` requires `binding_equals("held_item", "wood")` and `PickUpWoodAction` provides `binding("held_item", "wood")`, and the cook-and-eat chain (trace D), where `CookPotatoAction` gates itself to `INFINITY` when the campfire is too low, which is how hunger and fire maintenance become coupled in the search.

______________________________________________________________________

## 10. Correctness notes

- Under **BestCost + Dijkstra**, the planner returns a cost-optimal plan, where cost is the sum of the declared action costs. **FirstComplete** returns the first valid plan, which is not guaranteed to be cheapest.
- An action's own effect never satisfies its own preconditions; preconditions are validated against the state before the action runs.
- The search is sound but bounded: `max_depth`, the iteration budget, and the open-requirements cycle guard all trade completeness for frame-budget guarantees in a real-time game loop.
