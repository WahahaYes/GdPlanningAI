# Behaviors

What GdPlanningAI users rely on, one entry per behavior. Each entry has a slug, a scenario, and the tests that prove it; planner cases are fixtures in `addons/GdPlanningAI/rust/tests/scenarios/<slug>.toml`, example scenes are cases in `test/scenarios/scenes.cfg`. Every test proves one entry. A unit test that names none must test an algorithmic kernel and say why.

A bug fix adds a case to the test of the behavior it broke. If no entry fits, add the entry first.

## Planning results

### `plan-reaches-goal`

**Scenario:** Given an unmet goal and actions whose simulated effects meet it, when the agent plans, then it gets a chain of those actions in an order where each one's preconditions hold in the state the earlier ones leave.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/plan-reaches-goal.toml`, `test/integration/test_async_planner.gd`

### `satisfied-goal-plans-nothing`

**Scenario:** Given a goal that already holds, when the agent plans, then it gets an empty, successful plan for that goal and no search runs.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/satisfied-goal-plans-nothing.toml`, `test/integration/test_async_planner.gd`

### `unreachable-goal-fails`

**Scenario:** Given a goal no chain within `max_recursion` can meet (no action affects it, or a precondition nothing provides), when the agent plans, then the search ends with a failed plan instead of running on.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/unreachable-goal-fails.toml`, `test/integration/test_async_planner.gd`

### `cheapest-plan-wins`

**Scenario:** Given several chains that meet a goal, when the agent plans, then it gets the one with the lowest total cost, even if it is longer.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/cheapest-plan-wins.toml`

### `goals-by-priority`

**Scenario:** Given several goals, when the agent plans, then goals are tried from the highest reward down and the first one with a plan wins.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/goals-by-priority.toml`, `addons/GdPlanningAI/rust/tests/planner_integration.rs`

## Chaining and verification

### `requirements-chain-through-provisions`

**Scenario:** Given an action that requires a binding or fact, when the agent plans, then an action providing it is placed before it: `binding` meets `binding_exists`, `binding_equals` and (for objects in the group) `binding_in_set`, and `fact_wildcard("at_target")` meets every `at_target` fact, so Go To is planned in for each interaction. Provisions that already hold need no action.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/requirements-chain-through-provisions.toml`, `test/integration/test_requirements_provisions.gd`

### `plans-verified-forward`

**Scenario:** Given a candidate chain, when the planner replays it from the starting state, then the chain is rejected if any action's preconditions or requirements fail in the state just before it, including a need an earlier step met and a later step undid.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/plans-verified-forward.toml`

### `custom-preconditions-decide`

**Scenario:** Given an action with a custom precondition, when the planner considers it, then the callable decides, with the planner's bindings injected: false prunes the branch, and the callable is asked again in the state just before the action when the plan is verified.

**Proved by:** `addons/GdPlanningAI/rust/tests/scenarios/custom-preconditions-decide.toml`

### `precondition-semantics`

Unit tier: the built-in precondition evaluator is an algorithmic kernel.

**Scenario:** Given a built-in precondition on the agent, the world state or a world-object group, when it is evaluated, then it matches the [quick reference](AUTHORING_GUIDE.md#quick-reference-built-in-preconditions): ints and floats compare across types, floats within a tolerance, a missing property equals `null` and `""`, and a group check holds if any object in the group matches.

**Proved by:** `addons/GdPlanningAI/rust/src/plan_types.rs` (one table shared by the planner and the goal pre-filter), `addons/GdPlanningAI/rust/src/precondition.rs` (operator names)

## Scheduling

### `planning-off-main-thread`

**Scenario:** Given agents submitting plans, when the scheduler runs, then each search runs on a worker thread in slices bounded by `iteration_budget` and `time_slice_ms`, a cancelled or superseded search stops without delivering a result, and finished engines are freed off the main thread.

**Proved by:** `addons/GdPlanningAI/rust/tests/time_slice.rs`, `addons/GdPlanningAI/rust/tests/cancellation.rs`, `addons/GdPlanningAI/rust/src/scheduler.rs`, `test/integration/test_async_planner.gd`

### `simulations-run-once`

**Scenario:** Given a search, when several branches need the same action simulated from the same state, then its cost and effect callables run once and the answer is shared, including between discovery and verification.

**Proved by:** `addons/GdPlanningAI/rust/tests/planner_integration.rs`, `addons/GdPlanningAI/rust/tests/step_cache.rs`

### `planning-strategies`

**Scenario:** Given an agent config's planning strategy and `failure_backoff`, when plans succeed, fail or come back empty, then the agent plans again exactly when the strategy says, waits out the backoff after a failure, and submits nothing without an entity.

**Proved by:** `test/unit/test_failure_backoff.gd`, `test/unit/test_planning_strategy_switch.gd`

### `plan-state-cleaned-up`

**Scenario:** Given actions that keep per-run state with `set_state`, when a plan ends or a new plan replaces it, then that state is gone from the agent blackboard.

**Proved by:** `test/unit/test_gdpai_blackboard.gd`

## Authoring API

### `blackboard-api`

**Scenario:** Given a blackboard, when properties and world objects are written and read, then values keep their types, objects are wrapped in proxies found by group, and a clone is an independent copy for simulation.

**Proved by:** `test/unit/test_gdpai_blackboard.gd`, `test/unit/test_sim_object_proxy.gd`

### `goto-arrives-or-replans`

**Scenario:** Given an agent walking to a target with Go To, when it gets stuck, then it counts as arrived near the end of its path and fails otherwise, so the agent replans.

**Proved by:** `test/unit/test_goto_action.gd`

### `objects-offer-actions`

**Scenario:** Given an object that offers a pick-up action, when the agent plans, then the action provides `held_item` bound to that object and `is_food` only for objects in the `Food` group, and a freed object provides nothing.

**Proved by:** `test/unit/test_pickup_action.gd`

## Examples

### `example-agents-live`

**Scenario:** Given an example scene, when it runs headless, then it loads and settles, the hunger agent eats, the campfire agent fuels its fire and eats a cooked potato, agents in the large scene pair up on their home fires, and in the long runs every agent finishes plans, nobody starves and every fire keeps burning.

**Proved by:** `test/scenarios/scenes.cfg`, `test/integration/test_example_scenes.gd`

### `example-goals-step-up`

**Scenario:** Given the example hunger and fire goals, when an agent plans, then each plan asks for one meal or one fuel delivery of progress, never past sated or full, for the agent's home fire if it has one.

**Proved by:** `test/unit/test_hunger_goal.gd`, `test/unit/test_maintain_fire_goal.gd`

### `example-scenes-present`

**Scenario:** Given a 3D example scene, when it runs, then the camera rig cycles agents and shows its controls, single-agent scenes omit switching, and a burnt-out flame is hidden.

**Proved by:** `test/integration/test_example_scenes.gd`, `test/integration/test_campfire_flame_3d.gd`, `test/scenarios/scenes.cfg`

## Tooling

### `search-is-debuggable`

**Scenario:** Given the default log level (Info), when a plan completes, then one `RESULT` line summarizes it; at Debug the full search tree is recorded for `get_debug_tree`, with the failing need on each rejected node, and a stalled search is noted once per callback.

**Proved by:** `addons/GdPlanningAI/rust/tests/logging_levels.rs`, `addons/GdPlanningAI/rust/src/scheduler.rs` (stall notice), `test/unit/test_log_level_config.gd`

### `engine-matches-addon`

**Scenario:** Given the addon and its Rust library, when the autoload starts, then the library's version equals `plugin.cfg`, so a stale binary is caught.

**Proved by:** `test/unit/test_engine_version.gd`
