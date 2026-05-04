# Backward-Chaining GOAP Planner Plan

## Purpose

This document records the corrected planner architecture for GdPlanningAI. The planner is intended to be a GOAP-style backward-chaining planner, extended with simulation, requirements, and provisions.

The current Rust planner implementation behaves like a forward search. It starts from the current state, applies candidate actions, and checks whether the simulated result is closer to the goal. That structure makes requirement/provision chaining difficult and can produce temporally inverted branches such as `Eat -> Pickup` instead of `Pickup -> Eat`.

The intended architecture should start from the goal and work backward through actions that can satisfy currently open needs.

## Core Architecture

The planner should search backward from a desired goal state.

At any point in the search, a branch represents a partially built plan suffix plus a set of unresolved needs that must be true before that suffix can execute.

Conceptually:

```text
open needs:
  state preconditions that must become true
  structured requirements that must be provided

selected suffix:
  actions already chosen, stored in execution order
```

When the planner selects a new action, that action is inserted before the current suffix. The action must satisfy at least one currently open need.

For example:

```text
Goal:
  hunger <= 0

Select EatHeldFood because simulated effect can satisfy hunger <= 0.
Open needs now include:
  held_item exists

Select PickupFood because it provides held_item.
Open needs now include:
  PickupFood's preconditions and requirements

Select GoToFood because it provides at_target(food).

Final execution chain:
  GoToFood -> PickupFood -> EatHeldFood
```

## Role of Preconditions, Requirements, and Provisions

### Preconditions

Preconditions remain evaluative state checks.

Builtin preconditions can be included as open state needs during backward search. A branch is complete only when the initial state satisfies all remaining open preconditions.

Custom preconditions remain runtime/simulation guards unless paired with planner-readable requirements or provisions. The planner should not infer dependencies from arbitrary callbacks.

### Requirements

Requirements are structured dependencies introduced by actions in the current suffix.

When an action is selected, its requirements become open needs that must be satisfied by earlier actions or by initial provisions extracted from the initial agent/world state.

Example:

```text
EatHeldFood requires binding_exists("held_item")
```

Selecting `EatHeldFood` adds `held_item` as an open requirement.

### Provisions

Provisions are structured facts or bindings supplied by an earlier action for later actions.

Example:

```text
PickupFood provides binding("held_item", food_id)
```

A provision can satisfy an open requirement from the suffix. When the provider action is inserted before the suffix, the matched requirement is removed from the open set.

## Role of `simulate_effect`

`simulate_effect` should remain the planner-readable state effect mechanism.

The planner should use it to determine whether an action can satisfy an open state need such as:

```text
hunger <= 0
world_has_food == true
campfire_lit == true
```

However, `simulate_effect` should not be responsible for expressing symbolic dependencies. Dependencies should be declared through requirements and provisions.

This gives each channel a clear responsibility:

```text
simulate_effect:
  describes concrete state changes

requirements:
  describe what an action needs before it can execute meaningfully

provisions:
  describe what an action supplies for later actions
```

## Hypothetical Simulation

Some actions cannot reveal their useful state effect unless their requirements are present.

Example:

```text
EatHeldFood requires held_item.
Without held_item, simulate_effect should not reduce hunger.
```

To preserve `simulate_effect` as the effect oracle, the backward planner may run candidate classification simulation against a temporary hypothetical snapshot that satisfies the action's declared requirements.

This is not the same as authoring fake effects in user actions. The placeholder state is planner-owned and used only to answer:

```text
Could this action satisfy the open state need if its declared requirements were provided?
```

Final plans must still be validated through real forward simulation from the actual initial state.

## Backward Search Algorithm

For each goal, sorted by reward:

```text
root.open_preconditions = goal.desired_state
root.open_requirements = []
root.action_chain = []

search(root)
```

Search step:

```text
if initial state satisfies all open preconditions
and initial provisions satisfy all open requirements:
    forward-validate action_chain
    record valid plan
    return

choose one open need
find actions that can satisfy that need

for each candidate action:
    clone branch
    remove needs satisfied by candidate
    add candidate.preconditions to open_preconditions
    add candidate.requirements to open_requirements
    insert candidate at the front of action_chain
    recurse
```

An action is eligible only if it satisfies a currently open need:

```text
1. It satisfies an open state precondition through simulated effect.
2. It satisfies an open structured requirement through provisions.
```

The planner should not include an action merely because it has provisions or introduces requirements.

## Forward Validation and Costing

Backward search produces candidate chains. Each complete candidate chain should be validated by simulating forward from the real initial state.

Forward validation should:

```text
1. Start with the real initial agent/world snapshots.
2. Extract initial provisions.
3. For each action in execution order:
   - check dependent objects are valid
   - check validity checks
   - check preconditions against the current simulated snapshots
   - check requirements against accumulated provisions
   - call get_cost with the current simulated snapshots
   - reject infinite-cost actions
   - call simulate_effect
   - accumulate action provisions
4. Verify the final snapshots satisfy the goal.
5. Return total cost if valid.
```

This keeps search symbolic/dependency-driven while preserving simulation accuracy.

## Replacement for Current Plan Tree Semantics

The current `PlanTreeNode` extraction assumes root-to-leaf order is execution order. That assumption is incompatible with true backward chaining unless extracted paths are reversed.

The implementation should either:

```text
Option A:
  keep a tree but reverse root-to-leaf paths before validation/result delivery

Option B:
  replace the tree with explicit branch records containing action_chain in execution order
```

Option B is preferred because it makes the intended ordering explicit and reduces the chance of accidentally reintroducing forward-search semantics.

## Implementation Plan

### Phase 1: Preserve Current Behavior with Tests ✅ Completed

Add regression tests that expose the current ordering bug and expected backward behavior.

Test cases:

```text
Goal hunger satisfied by EatHeldFood.
EatHeldFood requires held_item.
PickupFood provides held_item.
Expected plan: PickupFood -> EatHeldFood.
Invalid plan: EatHeldFood -> PickupFood.
```

Also add a multi-step chain case:

```text
GoToFood provides at_target(food).
PickupFood requires at_target(food), provides held_item.
EatHeldFood requires held_item, satisfies hunger.
Expected plan: GoToFood -> PickupFood -> EatHeldFood.
```

Implemented in `addons/GdPlanningAI/rust/src/planner.rs` and covered by
`test/integration/test_requirements_provisions.gd`.

### Phase 2: Introduce Backward Branch State ✅ Completed

Add a branch representation that tracks:

```text
open_preconditions: Vec<PreconditionSpec>
open_requirements: Vec<RequirementSpec>
action_chain: Vec<i64>
total_lower_bound_cost or estimated_cost
```

The action chain should be stored in execution order. When a predecessor action is selected, insert it at the front.

Implemented with `PlanBranch`, `SearchContext`, and explicit `action_chain` storage in execution order.

### Phase 3: Implement Requirement/Provision Backward Matching ✅ Completed

Implement candidate selection for open requirements:

```text
open requirement -> actions with matching provisions
```

When an action is selected:

```text
remove requirements satisfied by its provisions
add action.requirements
add action.preconditions
```

Do not add actions just because they have provisions. They must satisfy an existing open requirement.

Implemented for `BindingExists`, `BindingEquals`, `BindingInSet`, and `Fact` matching. `BindingInSet` is context-aware and checks known world object group membership for object references or world object UIDs.

### Phase 4: Implement Simulated State-Need Matching ✅ Completed

Implement candidate selection for open preconditions using `simulate_effect`.

For each action candidate:

```text
clone an appropriate baseline snapshot
optionally apply hypothetical bindings/facts required by the action
call simulate_effect
check whether the open precondition is now satisfied
```

If the action satisfies the open state need:

```text
remove the satisfied precondition
add action.preconditions
add action.requirements
insert action at front of chain
```

Implemented with planner-owned hypothetical snapshots. Requirement-dependent state effects are classified by temporarily satisfying declared requirements before calling `simulate_effect`.

### Phase 5: Forward Validate Complete Chains ✅ Completed

When all open needs are satisfied by the initial state and initial provisions, run full forward validation/costing.

Reject chains if:

```text
an action dependency object is invalid
an action validity check fails
an action precondition fails
an action requirement is not satisfied by accumulated provisions
an action has infinite cost
the final state does not satisfy the selected goal
```

Only validated chains should be returned to GDScript.

Implemented final goal validation, precondition checks, requirement checks against accumulated provisions, cost callbacks, effect simulation, and provision accumulation.

### Phase 6: Pruning and Search Control ✅ Partially Completed

Add pruning once correctness is restored.

Recommended controls:

```text
max recursion depth
visited state keyed by open needs plus selected suffix
cost upper bound from best valid plan
candidate ordering by requirement specificity and estimated cost
provision/action indexes for faster lookup
```

Avoid broad inclusion rules such as:

```text
has provisions
introduces requirements
```

These recreate near-exhaustive search.

Implemented:

```text
max recursion depth
candidate ordering by estimated cost and deterministic action index
cost upper bound from best valid plan
branch-and-bound pruning when branch.estimated_cost >= best_valid_cost
```

Still pending:

```text
visited state keyed by open needs plus selected suffix
provision/action indexes for faster lookup
more precise lower-bound cost estimates
```

### Phase 7: Cleanup Old Forward Planner Artifacts ✅ Mostly Completed

Remove or rewrite concepts that belong to the accidental forward planner:

```text
active_requirements flowing down recursion
placeholder/deferred action nodes as executable plan members
two-pass same-depth requirement collection
plan extraction that treats backward-selected root-to-leaf order as execution order
```

`accumulated_provisions` should remain, but it belongs primarily in forward validation and in branch completion checks against initial provisions.

The old two-pass same-depth requirement collection and plan-tree-based extraction path were replaced by explicit backward branch search. `PlanTreeNode` still exists for older tests/types, but the active planner no longer builds plans through it.

## Current Implementation Status

Implemented and committed in:

```text
8683ded reimplement backward planner
d1e0536 backwards planner fixes
```

Current behavior:

```text
search starts from goal desired_state
branch stores open_preconditions, open_requirements, action_chain, bound_provisions, pending_effects, estimated_cost
candidate actions must satisfy an open requirement through provisions, directly satisfy an open state need through provider-bound simulate_effect, or make a pending state-effect claim whose requirements have possible providers
selected predecessor actions are inserted at the front of action_chain
selected predecessor provisions are added to bound_provisions
pending state-effect claims are re-simulated when new providers bind concrete requirement values
complete chains are forward-validated from real initial snapshots
search keeps the cheapest forward-validated plan found within max_depth
branches with estimated_cost >= best valid cost are pruned
```

Requirement/provision support:

```text
BindingExists:
  satisfied by a non-null, non-empty binding provision

BindingEquals:
  satisfied by an equal binding provision

BindingInSet:
  satisfied by a binding value that refers to a known world object in the requested group

Fact:
  satisfied by an exactly matching fact provision
```

Provider-bound hypothetical simulation support:

```text
BindingExists:
  inserts the concrete value from an initial provision or selected provider action

BindingEquals:
  inserts the required value

BindingInSet:
  inserts the concrete provider value that satisfies world object group membership

Fact:
  currently has no direct blackboard representation and is used only in provision matching
```

The planner no longer fabricates placeholder values to prove requirement-dependent effects. If an action cannot accurately simulate its effect until requirements are bound, it can create a pending state-effect claim. That claim remains unresolved until selected predecessor actions provide concrete provisions; the planner then re-simulates the dependent suffix action with those provider-bound values and only clears the pending need if the real effect satisfies the claimed precondition.

Validation status:

```text
make test-rust
make build-release
make test-godot
make lint-style
```

All passed after the latest planner hardening. The Godot suite includes 36 tests and 97 assertions.

## Open Questions

1. Should facts eventually have a blackboard representation for hypothetical simulation, or should they remain provision-only planner metadata?
2. Should `BindingExists` remain value-agnostic, or should common contracts prefer `BindingEquals` / `BindingInSet` for stronger provider selection?
3. Should `BindingInSet` support explicit named set registries in addition to world object group membership?
4. Should custom preconditions ever be allowed as open needs, or should they only be checked during forward validation?
5. Should the planner keep `PlanTreeNode` for debugging visualization, while using explicit branches for search?
6. What visited-state key should be used to suppress cycles without incorrectly pruning distinct useful chains?

## Decision Summary

The corrected architecture is:

```text
Backward search for structure and dependencies.
Requirements/provisions for symbolic dependency chaining.
simulate_effect for planner-readable state effects.
Hypothetical simulation only for classifying requirement-dependent effects.
Forward simulation for final validation and cost.
```

This preserves the original GOAP intent while extending it with structured requirements and provisions.
