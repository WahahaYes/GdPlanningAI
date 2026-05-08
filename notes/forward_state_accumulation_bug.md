# Forward State Accumulation Bug

## Problem

When forward state simulation was added to `backward_search`, the effect callback was invoked against the accumulated state **before** the action's requirements were satisfied. This caused incorrect state mutations.

### Root Cause

In `backward_search`, the original order was:

1. Clone branch
2. Insert action at front of chain
3. **Simulate effect on accumulated state** ← runs before requirements are met
4. Update open needs (resolve requirements, pending effects)

The effect simulation calls the GDScript callback with the current accumulated state. If the action has unmet requirements (e.g., `BindingInSet("held_item", "edible")`), the callback may still produce side effects based on default/empty property values.

### Concrete Example

Action `EatEdible`:
- Requirement: `BindingInSet("held_item", "edible")`
- Effect callback: `if agent.get_property("held_item") != null: agent.set_property("ate", true)`

When EatEdible is first selected as a suffix candidate (depth 0), the accumulated state is `{ate: false, held_item: ""}`. The effect callback checks `"" != null` — in GDScript, an empty string is **not** null, so this evaluates to `true`. The callback sets `ate = true` even though no real food item is held.

This corrupts the accumulated state. When `resolve_pending_effects` later checks if EatEdible's effect can satisfy the goal precondition `ate == true`, it sees `ate` is **already** true in the accumulated state. The before→after delta is zero, so the precondition is not considered "newly satisfied" and the pending effect never resolves.

### Impact

- `test_binding_in_set_requires_world_group_membership` fails: plan returns empty instead of `[PickupBanana, EatEdible]`
- Interactive examples break: agents ignore spawned food because the planner incorrectly believes the goal is already achieved

## Fix (Two Parts)

### Part 1: Reorder — simulate effect AFTER `update_open_needs`

Move the effect simulation to after `update_open_needs` so that `resolve_pending_effects` has a chance to bind provisions before the effect runs.

```
1. Clone branch
2. Insert action at front
3. Update open needs (resolve requirements, pending effects)
4. Simulate effect on accumulated state  ← moved here
5. Recurse
```

This ensures the accumulated state used for effect simulation reflects any newly bound provisions from the current action.

### Part 2: Guard — only simulate if requirements are met

Even after reordering, an action's effect should not be simulated if its own requirements are unsatisfied. Add a guard:

```rust
let requirements_met = action.requirements.is_empty()
    || requirements_satisfied_in_context(
        &action.requirements,
        &new_branch.bound_provisions,
        &new_branch.accumulated_world,
    );
if requirements_met {
    // simulate effect
}
```

This prevents the effect callback from running with incomplete/missing bindings. The effect will be properly simulated later via `resolve_pending_effects` when the requirements are eventually satisfied by a predecessor action's provisions.

### Why This Works

With both fixes, the EatEdible example flows correctly:

1. **Depth 0**: EatEdible selected as suffix. Requirements not met → effect **not** simulated. Accumulated state stays `{ate: false, held_item: ""}`.
2. **Depth 1**: PickupBanana selected as predecessor. `update_open_needs` binds `held_item=banana`, `resolve_pending_effects` creates hypothetical with `held_item=banana`, simulates EatEdible's effect → `ate` goes from false to true → pending effect resolved.
3. PickupBanana's effect simulated: `held_item=banana` set in accumulated state.
4. `is_complete` returns true. `forward_validate` confirms the chain `[PickupBanana, EatEdible]`.
