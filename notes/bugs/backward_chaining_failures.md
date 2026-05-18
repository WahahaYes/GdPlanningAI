# Bug: Action Effects Not Persisting in Discovery Snapshots

**Status**: Investigating (42/46 Tests Passing)
**Focused Tests**: `test_full_cooking_chain`, `test_preemptive_fire_maintenance`, `test_requirement_dependent_effect_uses_provider_bound_resimulation`

## Symptoms
1.  **[RESOLVED]** Inline test actions were missing "Action-Led Hypothetical Progress" logic.
2.  **[RESOLVED]** Agent-held items were ignored because `scheduler.rs` only looked at world objects for initial provisions.
3.  **[ACTIVE]** Campfire tests are failing because goals are being treated as `Custom` preconditions.
4.  **[ACTIVE]** `EatHeldFood` simulation shows it satisfies `hunger <= 10` in discovery, but the plan still fails to form or finds 0 actions in some contexts.

## Hypotheses

### 1. The "Custom Goal" Opaque Boundary (High Probability)
- **Description**: In `test_full_cooking_chain`, the goal is serialized as a `Custom` precondition rather than a `Builtin`. 
- **Impact**: The planner cannot perform "Discovery" simulation effectively if it doesn't know what property the goal is looking for. `EatHeldFood` satisfies `hunger`, but if the goal is just "Callable #55", the discovery phase doesn't know that hunger reduction is the path to success.
- **Action**: Check `Precondition.gd` and ensure it uses the `builtin` variant whenever possible.

### 2. Fuel-Check Cost Infinity
- **Description**: `CookPotatoAction` returns `INF` if fuel is low. If the `SimObjectProxy` lookup for the campfire fails in the planner's thread, the cost becomes infinite, pruning the branch.
- **Action**: Verify `campfire_ref` is correctly passed and found in the simulated world state.

### 4. Custom Precondition Context Loss (RESOLVED)
- **Description**: Custom callables in GDScript were receiving blackboards without the simulated bindings (like `held_item`) injected.
- **Resolution**: Updated `EvalCustomPrecond` callback to pass provisions and bindings. `scheduler.rs` now injects these into the temporary blackboard before the call. This allows complex custom logic to work during backward chaining.

## Final Remaining Challenges
1.  **Chaining via Facts**: actions like `CookPotato` provide a string "cooked_potato", but `EatHeldFood` requires a "Food" group member. String provisions don't have groups in simulation.
    - **Action**: Use a symbolic `is_food` fact to bridge the gap.
2.  **Cost Imbalance**: Agent chooses `Wander` over `Shake Tree`.
    - **Action**: Lower `Shake Tree` cost or ensure `Hunger` reward is high enough to justify the 100.0 cost interaction.
