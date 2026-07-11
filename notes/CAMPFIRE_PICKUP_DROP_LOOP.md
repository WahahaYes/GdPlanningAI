# Campfire 2D: Pick Up Wood / Drop Item Loop

**Date:** 2026-07-11  
**Source:** `campfire_2d_run.log` (first 30s of `examples/campfire_2d.tscn`)

## Observed Behavior

For the first ~15 seconds the agent repeatedly plans and executes:

1. `Go To → Pick Up Wood`
2. `Drop Item`

Then it starts over. The agent picks up a wood pile and immediately drops it, making no progress.

Log excerpt:

```text
RESULT: SUCCESS — [Go To → Pick Up Wood] cost=1.21
...
RESULT: SUCCESS — [Drop Item] cost=0.20
...
RESULT: SUCCESS — [Go To → Pick Up Wood] cost=0.57
...
RESULT: SUCCESS — [Drop Item] cost=0.20
```

## Why It Happens

The agent has two goals:

- `Hunger` — infeasible for the first ~15 seconds because `HungerGoal.get_desired_state` requires `hunger < current_hunger - 15`, which is impossible until `hunger` exceeds 15.
- `Maintain Fire` — the campfire starts at 100 fuel and only needs to stay above 60, so it is already satisfied for the first part of the run.

Because `Maintain Fire` is already satisfied, the planner should produce an empty plan. Instead, it finds the cheapest non-empty plan that does not break the satisfied `Maintain Fire` precondition. `Pick Up Wood` (cost 0.5) and `Drop Item` (cost 0.2) are the cheapest available actions, so they are selected and executed on repeat.

## Recommendation

Provide a valid `do_nothing` action so the planner has a concrete, zero-cost plan when no work is needed.

A minimal `do_nothing` action would:

- Have no preconditions.
- Have no effects.
- Return a very low cost (0.0 or a small epsilon).
- Be registered by `HungerBehaviorConfig`, `CampfireBehaviorConfig`, or a shared base behavior.

With a `do_nothing` action, the planner will choose it over `Pick Up Wood → Drop Item` when `Maintain Fire` is already satisfied and `Hunger` is infeasible.

## Files to Modify

- `examples/behaviors/campfire/campfire_behavior_config.gd` or `examples/behaviors/hunger/hunger_behavior_config.gd` to add the action.
- Optionally create `examples/behaviors/shared/do_nothing_action.gd` for reuse.

## Notes

- This is not a campfire-specific action design issue; the same loop can occur for any satisfied goal that has no empty plan.
- The `Maintain Fire` goal is not necessarily the only place this happens. The `Hunger` goal also returns failure for the first ~15 seconds, leaving `Maintain Fire` as the only goal the planner can satisfy, even though it is already satisfied.
