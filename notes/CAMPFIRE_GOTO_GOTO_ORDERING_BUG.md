# Campfire 2D: `Go To → Go To` Ordering Bug

**Date:** 2026-07-11  
**Source:** `campfire_2d_run.log` (first 30s of `examples/campfire_2d.tscn`)

## Observed Behavior

Whenever the planner builds a multi-step campfire plan it returns chains like:

- `Go To → Go To → Pick Up Wood → Add Fuel`
- `Go To → Go To → Dig Potato → Cook Potato → Eat Held Food`

Both `Go To` actions appear at the front of the chain, before the interaction they are meant to support. This is spatially wrong: the agent ends up navigating to the final target first, then walking away to interact, and the final `Add Fuel` / `Cook Potato` is left without a preceding `Go To`.

Example from the log (`Plan complete` at ~23.3s):

```text
RESULT: SUCCESS — [Go To → Go To → Pick Up Wood → Add Fuel] cost=3.76
Branches: 74 | Time: 230.9ms
```

## Why It Is a Problem

- `Add Fuel` requires `at_target(campfire)` and `held_item == "wood"`.
- `Pick Up Wood` requires `at_target(wood_pile)` and produces `held_item == "wood"`.
- The correct forward order should be `Go To(wood_pile) → Pick Up Wood → Go To(campfire) → Add Fuel`.
- The current ordering puts both `Go To` actions before the first interaction, so one `Go To` is effectively wasted and the final interaction has no preceding navigation.

## Likely Cause

The planner is inserting `Go To` provider actions when it resolves `at_target` requirements, but it is not placing them immediately before the consumer action that introduced the requirement. Instead, predecessor `Go To` actions are being pushed toward the front of the chain. This regresses the expected behavior described in the chain-position/occurrence migration (the `GoToAction` clone-for-plan and per-position binding work), but the final ordering still needs to be fixed.

## Reproduction

Run the 2D campfire scene with debug logging (`plugin.cfg` `log_level=3`):

```bash
 godot --headless --path . examples/campfire_2d.tscn
```

After the fire decays below 60, the planner will produce the `Go To → Go To → Pick Up Wood → Add Fuel` plan. The same pattern appears in the `Hunger` potato chain once `hunger` exceeds 15.

## Recommended Fix

- When inserting a predecessor `Go To` for an `at_target` requirement, place the `Go To` immediately before the action that consumes the requirement, not at the front of the whole chain.
- Verify that `GoToAction` clone instances are bound to the correct target per chain position and that forward validation uses the binding for that specific position.
- Add a regression test (or extend `test_bridge_injects_bindings_by_chain_position_for_repeated_goto`) that asserts a `[Go To, Pick Up, Go To, Drop/Use]` ordering for a wood/fuel chain.
