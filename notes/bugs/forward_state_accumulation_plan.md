# Forward State Accumulation in the Rust Planner

## Motivation

The current Rust planner is a pure backward search — it always references `ctx.initial_agent` and `ctx.initial_world` when checking preconditions, estimating costs, and creating hypothetical snapshots. This means:

- When EatHeldFood is selected as a suffix action, its effect is simulated against the **initial** state (where `held_item` is null), producing no hunger reduction.
- When PickupAction is later added as a predecessor, the planner must use `pending_effects` + `resolve_pending_effects` to re-simulate EatHeldFood.
- This re-simulation also uses the initial state, just with provisions injected — it doesn't reflect any other state changes from predecessor actions.

The old GDScript planner avoided this by passing accumulated state forward through recursion. Each level received the state after all predecessors' effects, so suffix actions could be meaningfully simulated once their predecessors were known.

## Design

Add `accumulated_agent` and `accumulated_world` snapshots to `PlanBranch`, representing the state after all actions in the chain so far have been applied. Each recursion level propagates this state forward.

Requirements/provisions remain for **binding propagation** (cases where an action needs "some object of class X" but its effect depends on the specific instance). Simple state-based precondition chains work via accumulation alone.

## Changes

### 1. Add accumulated state to `PlanBranch`

```rust
struct PlanBranch {
    // ... existing fields ...
    /// State after all actions in this branch have been simulated forward.
    accumulated_agent: BlackboardSnapshot,
    accumulated_world: BlackboardSnapshot,
}
```

### 2. Initialize from initial state

`PlanBranch::new()` takes `initial_agent` and `initial_world` and clones them:

```rust
fn new(
    goal_preconditions: &[PreconditionSpec],
    initial_provisions: &[ProvisionSpec],
    initial_agent: &BlackboardSnapshot,
    initial_world: &BlackboardSnapshot,
) -> Self {
    Self {
        // ... existing fields ...
        accumulated_agent: initial_agent.clone(),
        accumulated_world: initial_world.clone(),
    }
}
```

### 3. `is_complete` uses branch's accumulated state

Remove `agent`/`world` parameters. Check preconditions against `self.accumulated_agent`/`self.accumulated_world`:

```rust
fn is_complete(&self, initial_provisions: &[ProvisionSpec], request_tx: &Sender<CallbackRequest>) -> bool {
    if !self.pending_effects.is_empty() {
        return false;
    }
    let preconditions_ok = self.open_preconditions.is_empty()
        || self.open_preconditions.iter().all(|p| {
            eval_precondition(p, &self.accumulated_agent, &self.accumulated_world, request_tx)
        });
    let requirements_ok = self.open_requirements.is_empty()
        || requirements_satisfied_in_context(&self.open_requirements, initial_provisions, &self.accumulated_world);
    preconditions_ok && requirements_ok
}
```

### 4. `update_open_needs` checks against accumulated state

When checking if an action's preconditions are already satisfied, use the branch's accumulated state instead of `ctx.initial_agent`/`ctx.initial_world`:

```rust
let already_satisfied = eval_precondition(
    precond,
    &branch.accumulated_agent,   // was: ctx.initial_agent
    &branch.accumulated_world,   // was: ctx.initial_world
    ctx.request_tx,
);
```

### 5. `create_hypothetical_snapshot` starts from accumulated state

Instead of cloning `ctx.initial_agent`, clone the branch's accumulated state. This means the hypothetical includes all predecessor effects:

```rust
fn create_hypothetical_snapshot(
    base_agent: &BlackboardSnapshot,    // was: ctx.initial_agent
    base_world: &BlackboardSnapshot,    // was: ctx.initial_world
    requirements: &[RequirementSpec],
    bound_provisions: &[ProvisionSpec],
) -> Option<(BlackboardSnapshot, BlackboardSnapshot)> {
    // ...
    let mut hypo_agent = base_agent.clone();
    let hypo_world = base_world.clone();
    // ...
}
```

Callers pass `&branch.accumulated_agent`/`&branch.accumulated_world`.

### 6. `estimate_action_cost` uses accumulated state

Same change — pass accumulated state to `create_hypothetical_snapshot`.

### 7. `backward_search` propagates accumulated state forward

When creating a new branch for a candidate action:

```rust
// Clone parent's accumulated state
let mut new_accumulated_agent = branch.accumulated_agent.clone();
let mut new_accumulated_world = branch.accumulated_world.clone();

// Simulate the new predecessor action's effect on the clone
let (after_agent, after_world) = call_apply_effect(
    action.effect_callable_id,
    new_accumulated_agent,
    new_accumulated_world,
    ctx.request_tx,
);

// New branch gets the modified state
new_branch.accumulated_agent = after_agent;
new_branch.accumulated_world = after_world;
```

This happens BEFORE `update_open_needs`, so the accumulated state reflects the new action's effect when checking its preconditions.

### 8. `resolve_pending_effects` uses accumulated state

When re-simulating a pending effect claim, `bound_effect_satisfied_precondition_indices` will use the accumulated state (which now includes the provider action's effect). This is more correct than using initial state with injected provisions.

### 9. `forward_validate` unchanged

`forward_validate` already does forward accumulation correctly. It serves as the final verification pass.

## What Simplifies

With forward state accumulation:

- **`potential_bound_effect_satisfied_precondition_indices`** may become unnecessary. This function exists to speculatively check if an action COULD satisfy a precondition if some other action provided the right binding. With accumulated state, the planner naturally discovers this by trying the provider action first.

- **`pending_effects`** complexity reduces. Re-simulation uses real accumulated state rather than hypothetical provision injection.

- **`create_hypothetical_snapshot`** becomes simpler — it's just cloning accumulated state + injecting binding values, rather than starting from initial state each time.

## What Stays

- **Requirements/provisions** remain for binding propagation. When EatHeldFood needs to know WHICH specific item was picked up (to determine hunger restoration amount), provisions carry that binding value.
- **`pending_effects`** still needed for actions selected before their binding providers. But re-simulation is more correct.
- **`forward_validate`** remains as the final verification pass.

## Risk: Channel Round-Trips

Each `call_apply_effect` requires a synchronous channel round-trip to the main thread. With forward accumulation, we call it once per candidate action per recursion level. This is the same cost as the current `bound_effect_satisfied_precondition_indices` which also calls `call_apply_effect`. No regression expected.

## Implementation Order

1. Add `accumulated_agent`/`accumulated_world` to `PlanBranch`, update `new()` signature
2. Update `is_complete()` to use self.accumulated_*
3. Update `update_open_needs()` precondition check
4. Update `create_hypothetical_snapshot()` to accept base state params
5. Update `estimate_action_cost()` to pass accumulated state
6. Update `bound_effect_satisfied_precondition_indices()` to pass accumulated state
7. Update `backward_search()` to simulate effect and propagate accumulated state
8. Update all call sites in `run_plan()`
9. Run tests, verify nothing breaks
10. Evaluate whether `potential_bound_effect_satisfied_precondition_indices` can be removed
