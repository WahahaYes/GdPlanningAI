# Planner Custom Precondition Bug

## Root Cause

The Rust planner (`planner.rs`) fails to evaluate **custom preconditions** (GDScript callbacks) in two critical locations, causing multi-action chains to fail. Both locations use `PreconditionSpec::evaluate_builtin()` which returns `None` for `Custom` variants, and `unwrap_or(false)` treats them as unsatisfied.

### Bug 1: `is_complete()` — line 68

```rust
let preconditions_ok = self.open_preconditions.is_empty()
    || self.open_preconditions.iter().all(|p| {
        p.evaluate_builtin(agent, world).unwrap_or(false)  // BUG: custom → false
    });
```

Custom preconditions in `open_preconditions` always cause `is_complete` to return `false`, so the branch never terminates.

### Bug 2: `update_open_needs()` — line 614

```rust
let already_satisfied = precond.evaluate_builtin(
    ctx.initial_agent,
    ctx.initial_world,
).unwrap_or(false);  // BUG: custom → false
```

When adding an action's preconditions as new open needs, custom preconditions are always treated as unsatisfied by the initial state, even when they are actually satisfied. This adds spurious open preconditions that can never be resolved.

### How the Old GDScript Planner Handled This

The old `plan.gd` used `Precondition.is_satisfied` flag tracking:
- `_evaluate_goals()` called `condition.evaluate()` which handled both builtin and custom
- `_is_goal_satisfied()` just checked the `is_satisfied` flag
- `copy_for_simulation()` preserved the flag

### The Fix

Replace `evaluate_builtin().unwrap_or(false)` with `eval_precondition()` in both locations. `eval_precondition` already handles both builtin and custom preconditions via the callback channel.

### Impact

This bug breaks any multi-action chain where actions have custom preconditions. The hunger example's Pickup → Eat chain fails because:
1. EatHeldFood has custom precondition `hunger > 0` 
2. PickupAction has custom precondition `has_empty_hands`
3. Both are incorrectly added to `open_preconditions` and can never be resolved
4. `is_complete` never returns true
5. Planner exhausts search and falls back to Wander goal

## Fix Status

**Fixed.** Two locations changed in `planner.rs`:
- `is_complete()`: now accepts `request_tx` and uses `eval_precondition()` instead of `evaluate_builtin().unwrap_or(false)`
- `update_open_needs()`: uses `eval_precondition()` instead of `evaluate_builtin().unwrap_or(false)` when checking if action preconditions are already satisfied by initial state

**Test results:** 37/37 Godot tests passing, 75/75 Rust tests passing.
