# Requirements/Provisions: Necessary Architecture or Bug Workaround?

## The Question

Was the requirements/provisions system a necessary architectural addition, or was it a workaround for the custom precondition evaluation bug we just fixed?

## What the Old Codebase Reveals

The pre-Rust-migration codebase (`GdPlanningAI-main`) had **no requirements/provisions system at all**. The `Action` base class only exposed:

- `get_preconditions()` — evaluative state checks
- `get_validity_checks()` — static validity guards
- `simulate_effect()` — forward state mutation
- `reverse_simulate_effect()` — backpropagation fixup for suffix actions

### How the Old Planner Chained Actions

The old planner (`plan.gd`) was a **regression (backward) planner with forward state accumulation** — a bidirectional approach:

- **Goal decomposition flows backward**: `desired_state` starts as the goal's preconditions. Each selected action's own preconditions get appended, forming sub-goals. Classic backward chaining.
- **State flows forward**: Each recursion level receives the state AFTER the previous action's effects were applied (`sim_blackboard`/`sim_world_state` are passed down).
- **`prior_actions` are suffix actions**: Already selected, will execute later. `reverse_simulate_effect` lets them fix up their effects once predecessor state is known.

Algorithm:
1. Clone agent + world state
2. Try each action: simulate effect on clone
3. Call `reverse_simulate_effect` on prior (suffix) actions to fix up their effects
4. Check if goal preconditions are satisfied against post-action state
5. If progress made, add action's preconditions to desired_state and recurse with modified state

This worked because the accumulated state at each recursion level reflects all predecessors' effects. Action B's preconditions are checked against the state AFTER Action A's effects. No explicit dependency tracking needed.

The `reverse_simulate_effect` mechanism (docstring from `action.gd:75-81`):
> "When <eat> is simulated, the agent isn't holding anything so we don't know what would've been eaten. But after <pickup> is simulated, we can refer back to the agent to figure out the object then determine how many hunger points that food is going to restore."

This was the old way of handling binding propagation — ad-hoc, callback-driven, and dependent on forward simulation order.

### Old SampleFoodAction (EatFood equivalent)

```gdscript
func get_preconditions() -> Array[Precondition]:
    return []  # No preconditions at all!

func simulate_effect(agent_blackboard, world_state):
    super(agent_blackboard, world_state)  # SpatialAction teleports agent to target
    var hunger = agent_blackboard.get_property("hunger")
    hunger += food_item.hunger_value  # Direct reference to food_item field
    agent_blackboard.set_property("hunger", hunger)

func reverse_simulate_effect(agent_blackboard, world_state):
    pass  # Empty — no backpropagation needed
```

The old `SampleFoodAction` directly referenced `food_item.hunger_value` — it knew the exact food item at construction time because `SpatialAction` bundled the target reference.

## Why the Rust Planner Needed Something New

Both old and new planners are **backward/regression** planners. The critical difference is:

| | Old (GDScript) | New (Rust) |
|---|---|---|
| Goal decomposition | Backward | Backward |
| State reference | **Accumulated forward** (modified state passed down each recursion) | **Always initial** (`ctx.initial_agent`) |
| Chain validation | Natural — state reflects all predecessors | Requires explicit dependency expression |

The Rust planner (`planner.rs`) always checks preconditions and simulates effects against `ctx.initial_agent` and `ctx.initial_world`. It never passes modified state down through recursion. This means:

- When Action B (suffix) is selected to satisfy a goal, the planner can't just check Action A's preconditions against "the state before Action A" — because there IS no accumulated state.
- The planner needs a separate mechanism to express "Action A provides X which Action B needs" — hence requirements/provisions.

This is the same problem the old planner solved with forward state accumulation + `reverse_simulate_effect`, just solved differently.

## What the Custom Precondition Bug Masked

The `evaluate_builtin().unwrap_or(false)` bug meant that **even simple builtin precondition chains failed**. For example, if EatHeldFood expressed "held_item must exist" as a `HasProperty("held_item")` builtin precondition:

1. The planner would check if `HasProperty("held_item")` is satisfied by initial state → correctly false
2. It would add `HasProperty("held_item")` to open preconditions
3. PickupAction's effect sets `held_item` → `bound_effect_satisfied_precondition_indices` would detect this
4. Chain would form: PickupAction → EatHeldFood

But with the bug, step 3 would fail for any custom preconditions, and step 4 (`is_complete`) would also fail. This made it seem like **no chaining worked at all** without requirements/provisions.

## What Requirements/Provisions Actually Solve

Even with the bug fixed, requirements/provisions solve problems that builtin preconditions cannot:

### 1. Binding Value Propagation
A `HasProperty("held_item")` precondition only checks **existence**. It doesn't tell the planner **which specific item** is held. EatHeldFood needs to know the item ID to determine how much hunger to restore (20 for banana vs 5 for apple). Requirements/provisions carry the actual binding value through the chain.

### 2. Fact-Based Relations
`at_target(tree)`, `door_open(door)`, `chest_unlocked(chest)` — these are relational facts between entities. They don't map cleanly to simple blackboard property checks. Requirements/provisions provide a structured way to express and match these.

### 3. Set Membership / World Knowledge
`held_item must be in group "edible"` — requires checking against world object groups. The `BindingInSet` requirement type encodes this as planner-readable metadata.

### 4. Efficiency
Matching a provision against a requirement is a simple struct comparison. Full simulation (clone state → call GDScript callback → compare before/after) requires channel round-trips to the main thread. For large action sets, provision matching is significantly cheaper.

### 5. Separation of Concerns
Preconditions answer "can this action run in this state?" Requirements answer "what must be made true earlier?" Provisions answer "what does this action make available for later?" These are different questions that deserve different mechanisms.

## Verdict

**The requirements/provisions system is a legitimate architectural component**, not just a bug workaround. It fills the gap left by removing forward state accumulation from the regression planner. The old planner had two mechanisms for chaining: (1) forward state accumulation through recursion, and (2) `reverse_simulate_effect` for binding propagation. The Rust planner replaced both with requirements/provisions — a more structured, planner-readable approach.

However, the system was **designed under inflated urgency**. The custom precondition bug made it seem like NOTHING worked without requirements/provisions, leading to:
- More complexity than strictly needed (e.g., `pending_effects`, `potential_bound_effect_satisfied_precondition_indices`)
- A sense that requirements/provisions were a **replacement** for preconditions rather than a **complement**

With the bug fixed:
- Simple state-based chains work via builtin preconditions alone
- Requirements/provisions can be seen as an **enhancement** for complex dependency patterns
- The system could potentially be simplified (e.g., some `pending_effects` machinery may be redundant now)

### Recommendation

Keep the requirements/provisions system but consider streamlining:
1. Review whether `pending_effects` / `potential_bound_effect_satisfied_precondition_indices` are still needed now that builtin precondition chaining works
2. Ensure documentation clearly positions requirements/provisions as an **advanced feature** for binding/fact propagation, not as a prerequisite for all action chaining
3. Simple cases (like `HasProperty` preconditions) should work without any requirements/provisions declarations
