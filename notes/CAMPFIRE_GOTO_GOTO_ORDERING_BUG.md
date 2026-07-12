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

## Investigation Findings

The root cause is in `addons/GdPlanningAI/rust/src/planner/engine.rs`, in the expansion step of `PlannerEngine::step_search`. Every candidate action is currently inserted with:

- `new_branch.action_chain.insert(0, cand.action_idx);`
- `new_branch.action_costs.insert(0, discovery_cost);`
- `new_branch.shift_positions(1);`

This always prepends the predecessor to the front of the chain. The position-occurrence infrastructure is otherwise correct, so the bug is purely in the insertion point.

The chain-position/binding machinery is already in place:

- `GoToAction` (`addons/GdPlanningAI/scripts/refcounteds/goto_action.gd`) provides `ProvisionSpec.fact_wildcard("at_target")` and implements `clone_for_plan()` and `inject_binding(...)`.
- `GdPAIAgent` (`addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd`) groups `action_bindings` by chain position and injects them into each cloned action for the matching plan step.
- `PlanBranch` (`addons/GdPlanningAI/rust/src/planner/types.rs`) tracks `open_requirements` and `action_bindings` with chain positions.

A second issue was found in `process_simulation` (`engine.rs`): after `simulate_action`, `open_requirements` are cleared with `provision_satisfies_requirement` using the raw `FactWildcard` provision. That function matches any `at_target(...)` requirement by name, so one `Go To` to a single target can clear every later `at_target` requirement. Forward validation should use the concrete binding for the current chain position to build a concrete `ProvisionSpec::Fact` before matching.

## Implementation Plan

1. **`planner/types.rs`**: Change `PlanBranch::shift_positions` so it only increments positions `>= insert_pos`, and take the start position as a parameter.
2. **`planner/engine.rs` expansion block**:
   - Compute `insert_pos` from the minimum position of all requirements and preconditions the candidate satisfies.
   - Insert `cand.action_idx` and `discovery_cost` at `insert_pos`.
   - Call `new_branch.shift_positions(insert_pos, 1)` instead of `shift_positions(1)`.
   - Push new preconditions and requirements at `insert_pos` instead of `0`.
   - Add the provider binding at `insert_pos` and consumer bindings at the shifted consumer positions.
3. **`planner/engine.rs` `process_simulation`**:
   - For `ProvisionSpec::FactWildcard` provisions, look up the current chain-position binding in `current_bindings`.
   - If a value exists, build a concrete `ProvisionSpec::Fact` with those args before clearing `open_requirements`.
   - Keep the wildcard fallback for legacy string-only `at_target` requirements with no injectable value.
4. **Regression test**:
   - Add a test in `test/integration/test_campfire_example_smoke.gd` or `test/integration/test_requirements_provisions.gd` that asserts a `[Go To, Pick Up, Go To, Add Fuel]` (or `[Go To, Dig, Go To, Cook]`) action chain for a multi-step campfire/hunger scenario.
5. **Validation**:
   - Run `make test-rust` for Rust unit tests.
   - Run `make build-release` to update the GDExtension binary.
   - Run `make test-godot` to verify the existing suite and the new regression test.

## Implementation Status

Implemented on 2026-07-11.

- `planner/types.rs` — `PlanBranch::shift_positions` now takes a start position and only shifts entries at or after that position.
- `planner/engine.rs` — expansion inserts predecessor actions at the earliest consumer position; forward validation concretizes `FactWildcard` provisions using the current chain-position binding.
- `test/integration/test_requirements_provisions.gd` — added `test_goto_wildcard_orders_two_goto_for_pickup_and_use`.

Validation passed: `make test-rust`, `make -C addons/GdPlanningAI/rust build-release`, `make test-godot`, `make lint-style`.

## Update 2026-07-12

During final verification `test-godot` reported `Agent should plan when hungry with fire available, but got empty plan` for `test_full_cooking_chain`. `HEAD` passed `test-godot`, so this was a regression introduced by the working-tree changes.

### Root cause

The `process_simulation` forward-validation work added two checks:

- Re-evaluation of `action.preconditions` against the current simulated state.
- Forward validation of `action.requirements` via `requirement_holds_in_state`.

The second check broke the Hunger chain. `Cook Potato` has both:

- a **requirement** `held_item == "potato"`
- a **provision** `held_item == "cooked_potato"`

`PlanBranch::action_bindings` stores both the provider (output) and consumer (input) bindings at the same chain position. When `process_simulation` reached `Cook` it collected `current_bindings` containing both:

```text
[("held_item", "cooked_potato"), ("held_item", "potato"), ("is_food", []), ("at_target", <campfire>), ...]
```

`requirement_holds_in_state` only inspected the first matching binding, saw `cooked_potato`, and concluded `held_item == "potato"` was false. This invalidated the only valid Hunger plan.

### Fix

`addons/GdPlanningAI/rust/src/requirement.rs`: `requirement_holds_in_state` now checks **all** bindings matching the binding name for `BindingExists`, `BindingEquals`, and `BindingInSet` (any-of semantics), rather than returning early on the first match. This lets `Cook`’s `held_item == "potato"` requirement be satisfied by the `potato` input binding even though `cooked_potato` is also present as the output binding.

### Files changed in this session

- `addons/GdPlanningAI/rust/src/requirement.rs` — `requirement_holds_in_state` uses any-of binding matching.
- `addons/GdPlanningAI/rust/src/planner/engine.rs` — `process_simulation` forward-validates `action.preconditions` and `action.requirements`; added debug logging to trace `Verifying` simulation results and requirement failures.
- `addons/GdPlanningAI/rust/src/planner/expander.rs` — `find_candidates` debug logging; skips wildcard candidates with pending preconditions.
- `addons/GdPlanningAI/rust/src/planner/types.rs` — `PlanBranch::shift_positions` now takes a start position and shifts only entries at or after it.
- `addons/GdPlanningAI/rust/src/snapshot.rs` — added `get_object_by_group` and `get_object_by_instance_id` helpers.
- `test/integration/test_requirements_provisions.gd` — added regression test for requirement/provision chaining.

### Validation after this session

- `make test-rust` — passed
- `make -C addons/GdPlanningAI/rust build-release` — passed
- `make test-godot` — passed (all Godot integration tests)
