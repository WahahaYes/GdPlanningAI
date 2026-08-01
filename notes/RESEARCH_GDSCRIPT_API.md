# Research: GDScript API Layer

**Date**: 2026-08-01 **Source**: `addons/GdPlanningAI/` — audited via explore agent (bg_959773e3) **Status**: Research dump — user-facing API map for docs/CODEBASE_OVERVIEW.md

______________________________________________________________________

## File inventory (29 scripts)

| Path | class_name | Extends | |------|------------|---------| | `scripts/refcounteds/action.gd` | `Action` | RefCounted | | `scripts/refcounteds/goal.gd` | `Goal` | RefCounted | | `scripts/refcounteds/goto_action.gd` | `GoToAction` | Action | | `scripts/refcounteds/property_updater.gd` | `PropertyUpdater` | RefCounted | | `scripts/refcounteds/precondition.gd` | `Precondition` | RefCounted | | `scripts/refcounteds/precondition_builtin.gd` | `PreconditionBuiltin` | Precondition | | `scripts/refcounteds/precondition_custom.gd` | `PreconditionCustom` | Precondition | | `scripts/refcounteds/precondition_custom_with_deps.gd` | `PreconditionCustomWithDeps` | Precondition | | `scripts/refcounteds/requirement_spec.gd` | `RequirementSpec` | RefCounted | | `scripts/refcounteds/requirement_spec_binding_equals.gd` | `RequirementSpecBindingEquals` | RequirementSpec | | `scripts/refcounteds/requirement_spec_binding_exists.gd` | `RequirementSpecBindingExists` | RequirementSpec | | `scripts/refcounteds/requirement_spec_binding_in_set.gd` | `RequirementSpecBindingInSet` | RequirementSpec | | `scripts/refcounteds/requirement_spec_fact.gd` | `RequirementSpecFact` | RequirementSpec | | `scripts/refcounteds/provision_spec.gd` | `ProvisionSpec` | RefCounted | | `scripts/refcounteds/provision_spec_binding.gd` | `ProvisionSpecBinding` | ProvisionSpec | | `scripts/refcounteds/provision_spec_fact.gd` | `ProvisionSpecFact` | ProvisionSpec | | `scripts/refcounteds/provision_spec_fact_wildcard.gd` | `ProvisionSpecFactWildcard` | ProvisionSpec | | `scripts/nodes/gdpai_agent.gd` | `GdPAIAgent` | Node | | `scripts/nodes/gdpai_world_node.gd` | `GdPAIWorldNode` | Node | | `scripts/nodes/gdpai_object_data.gd` | `GdPAIObjectData` | Node | | `scripts/nodes/gdpai_location_data.gd` | `GdPAILocationData` | GdPAIObjectData | | `scripts/nodes/gdpai_interactable.gd` | `GdPAIInteractable` | GdPAIObjectData | | `scripts/resources/gdpai_behavior_config.gd` | `GdPAIBehaviorConfig` | Resource | | `scripts/resources/gdpai_agent_config.gd` | `GdPAIAgentConfig` | Resource | | `scripts/gdpai_blackboard_plan.gd` | `GdPAIBlackboardPlan` | Resource | | `scripts/gdpai_rust_bridge.gd` | `GdPAIRustBridge` | RefCounted | | `gdpai_utils.gd` | `GdPAIUTILS` | Object | | `gdpai_autoload.gd` | *(none — autoload singleton `GdPAIAutoload`)* | Node | | `plugin.gd` | *(none — EditorPlugin)* | EditorPlugin |

Rust-exposed classes (registered via `gdplanningai.gdextension`, no `.gd`): `GdPAIBlackboard`, `SimObjectProxy`, `GdPAIPlanScheduler`.

## Node classes

### GdPAIAgent (gdpai_agent.gd) — the central runtime

- `@export var entity: Node`, `@export var config: GdPAIAgentConfig`.
- Public: `blackboard`, `world_node`, `goals`, `self_actions`, `property_updaters`, `get_current_goal()`, `get_current_plan()`, `get_current_plan_step()`, `set_planning_strategy(strategy, interval=0.5)`, `manually_start_plan()`.
- `_ready`: build agent blackboard from `config.blackboard_plan`, set `entity` + `GDPAI_OBJECTS` (children in GdPAIObjectData group under entity), apply behavior configs (`apply_to_agent`), find world node, create bridge.
- `_process(delta)`: run property updaters; if CONTINUOUS + plan done + not waiting → `_start_plan_async()`; `_execute_plan(delta)`.
- `_start_plan_async`: refresh GDPAI_OBJECTS, `all_actions = self_actions + _collect_worldly_actions()`, bump `_plan_generation`, then `scheduler.submit_plan(self, blackboard, world_node.get_world_state(), _bridge.serialize_actions(all_actions), _bridge.serialize_goals(goals, self), config.max_recursion, config.iteration_budget)`.
- `_on_plan_ready(result)`: generation-guarded (discards stale); `deserialize_plan_result` → `{action_chain, bindings_by_position}`; sets `_current_goal = goals[result.goal_index]`.
- Execution phases: **pre** (all `pre_perform_action`, any FAILURE aborts), **action** (`perform_action`; FAILURE aborts / RUNNING stays / SUCCESS advances), **post** (all `post_perform_action` — guaranteed cleanup).

### GdPAIWorldNode (gdpai_world_node.gd)

- `@export var blackboard_plan`; `@onready var world_state`.
- `get_world_state()` re-scans the scene tree each call: every node in group `"GdPAIObjectData"` → `world_state` under `"GDPAI_OBJECTS"`.

### GdPAIObjectData (gdpai_object_data.gd) — user subclasses

- `@export var entity: Node`. `_init` auto-adds groups (don't override).
- Override: `get_group_labels() -> Array[String]` (default `["GdPAIObjectData"]`), `get_provided_actions() -> Array[Action]`, `get_sim_properties() -> Dictionary`.

### GdPAILocationData (gdpai_location_data.gd) / GdPAIInteractable (gdpai_interactable.gd)

- LocationData: `location_node_2d` / `location_node_3d` (set one); computed `position` / `rotation`. Groups `["GdPAILocationData", "GdPAIObjectData"]`.
- Interactable: `max_interaction_distance = 2` (≤0 disables), `max_drift_from_plan = -1` (spatial validity vs planning-time position). Groups `["GdPAIInteractable", "GdPAIObjectData"]`.

## RefCounted extension points

### Action (action.gd)

- `enum Status { FAILURE, RUNNING, SUCCESS }`; `var chain_position: int = -1`.
- Planning-time overrides (background thread, no await / no scene-tree): `get_validity_checks() -> Array[Precondition]`, `get_action_cost(agent_bb, world_bb) -> float` (return INF to skip), `get_preconditions() -> Array[Precondition]`, `get_requirements() -> Array[RequirementSpec]`, `get_provisions() -> Array[ProvisionSpec]`, `simulate_effect(agent_bb, world_bb) -> void` (mutate in place).
- Execution-time overrides (main thread): `pre_perform_action(agent) -> Status`, `perform_action(agent, delta) -> Status`, `post_perform_action(agent) -> Status` (guaranteed cleanup), `clone_for_plan() -> Action` (only if mutable instance state), `get_title()`, `get_description()`.
- State helpers (use, don't override): `set_state/get_state/erase_state/ has_state` (namespaced on agent blackboard via `instance_id + "_" + chain_position + "_" + key`).

### Goal (goal.gd)

- `compute_reward(agent) -> float` (dynamic urgency), `get_desired_state(agent) -> Array[Precondition]`, `get_title()`, `get_description()`.

### PropertyUpdater (property_updater.gd)

- `update_properties(agent, delta) -> void` (every frame), `initialize(agent) -> void` (once when applied).

### GoToAction (goto_action.gd)

- `ARRIVAL_THRESHOLD_2D = 8.0`, `ARRIVAL_THRESHOLD_3D = 0.1`; `static find_nav_agent(entity) -> Node`; `target_location: GdPAILocationData`.
- `inject_binding(fact_name, value)` — the binding-injection protocol (for `"at_target"` sets `target_location` from `value[0]`).
- Provisions: `[ProvisionSpec.fact_wildcard("at_target")]`.
- Sim: teleports simulated agent location to target.

## Precondition system

### Precondition (precondition.gd) — base + factories

- `to_bridge_dict()` abstract (base errors).
- `static custom(fn)` / `static custom_with_deps(fn, deps)`.
- Agent: `agent_has_property`, `agent_property_{not_equal_to, greater_than, geq_than, less_than, leq_than, equal_to}`.
- WorldState: `world_state_has_property`, `world_state_property_{greater_than, geq_than, less_than, leq_than, equal_to}`.
- WorldObjectProxy: `world_object_property_{equal_to, not_equal_to, greater_than, geq_than, less_than, leq_than}`, `world_object_has_property`.
- Custom helpers: `agent_has_object_data_of_group`, `world_state_has_object_data_of_group`, `check_is_object_valid`.

### PreconditionBuiltin (precondition_builtin.gd)

- `enum Target { AGENT, WORLD_STATE, WORLD_OBJECT_PROXY }`, `enum Op { HAS_PROPERTY, EQUAL, NOT_EQUAL, GT, GTE, LT, LTE }`.
- Bridge dict: `{target, operation, property_name, value}` (+ `group`/`property` for WORLD_OBJECT_PROXY). Evaluated natively in Rust (no callback).

### PreconditionCustom (precondition_custom.gd)

- `eval_func: Callable`; bridge `{operation: "custom_callback", eval_callable}` — invoked on the main thread via scheduler callbacks.

### PreconditionCustomWithDeps (precondition_custom_with_deps.gd)

- `eval_func`, `dependent_objects`; false if any dep freed; bridge adds `dependent_object_ids` for Rust liveness validation.

## Requirement / Provision specs

### RequirementSpec — static factories

- `binding_exists(name)`, `binding_equals(name, value)`, `binding_in_set(name, set_name)`, `fact(name, args=[])`.
- Bridge kinds: `binding_equals`, `binding_exists`, `binding_in_set`, `fact`.

### ProvisionSpec — static factories

- `binding(name, value)`, `fact(name, args=[])`, `fact_wildcard(name)`.
- Bridge kinds: `binding`, `fact`, `fact_wildcard`.
- `fact_wildcard` satisfies ANY same-name fact requirement regardless of args — how `GoToAction`'s `at_target` satisfies all location requirements.

## Resources / Config

### GdPAIAgentConfig (gdpai_agent_config.gd)

- `enum PlanningStrategy { CONTINUOUS, ON_INTERVAL, ON_DEMAND, ON_INTERVAL_FORCED }`.
- Exports: `planning_strategy = CONTINUOUS`, `planning_interval = 0.5`, `max_recursion = 100` (min 1 enforced), `iteration_budget = 20000` (min 100 enforced), `blackboard_plan`, `behavior_configs`.
- Strategy behavior: CONTINUOUS replans each frame when plan done; ON_INTERVAL on timer when plan done; ON_INTERVAL_FORCED every tick regardless; ON_DEMAND never auto — call `manually_start_plan()`.

### GdPAIBehaviorConfig (gdpai_behavior_config.gd)

- `apply_to_agent(agent)` calls `_populate` fresh each time (shared-resource- safe), appends goals/self_actions, calls `updater.initialize(agent)`.
- Override `_populate(goals, actions, updaters)`.

### GdPAIBlackboardPlan (gdpai_blackboard_plan.gd)

- `@export var blackboard_backend: Dictionary`; `generate_blackboard() -> GdPAIBlackboard` (set_dict + ensure `GDPAI_OBJECTS` key).

## Plugin & autoload

### plugin.gd

- `@tool extends EditorPlugin`; `_enter_tree` registers autoload singleton `GdPAIAutoload` → `gdpai_autoload.gd`. Only editor registration.

### gdpai_autoload.gd (singleton GdPAIAutoload)

- `_ready`: creates `GdPAIPlanScheduler.new()`, applies log level from `plugin.cfg` (`log_level`: 0=Error..3=Debug; currently 3).
- `_process`: `_scheduler.process_callbacks()` — drains main-thread callbacks from background planner threads. `get_scheduler()`.

## Rust bridge (gdpai_rust_bridge.gd)

"Not intended for direct use — GdPAIAgent owns and calls this internally."

- `serialize_actions(actions) -> Array[Dictionary]`: per action `{name, cost_callable: Callable(action,"get_action_cost"), effect_callable: Callable(action,"simulate_effect"), preconditions, validity_checks, requirements, provisions}`.
- `serialize_goals(goals, agent) -> Array[Dictionary]`: `{name, reward: compute_reward(agent), desired_state}`.
- `deserialize_plan_result(result, actions) -> Dictionary`: reads `action_chain` (indices into submitted array), `action_bindings` (`[chain_position, fact_name, values]` of instance IDs) → groups into `bindings_by_position`; clones actions via `clone_for_plan()`. Returns `{action_chain, bindings_by_position}`.

## Blackboard & proxies (Rust classes, GDScript usage)

- `GdPAIBlackboard`: `set/get/erase/has_property`, `set_dict`, `get_object_for` (→ SimObjectProxy snapshot; writes reflect into simulation), `get_proxy_in_group`, `get_proxies_in_group`, `get_node_in_group` (live node).
- `GDPAI_OBJECTS` reserved key on both agent and world blackboards holds arrays of `GdPAIObjectData` scene nodes. World node repopulates per call; agent repopulates before each async submit.

## GdPAIUTILS (gdpai_utils.gd)

- `get_child_of_type(node, _class)` recursive; `get_children_in_group(node, group)`.

## Must-NOT-override / internal surface

- `Action.set_state/get_state/erase_state/has_state` — use, don't override.
- `Precondition/RequirementSpec/ProvisionSpec.to_bridge_dict` — implemented by concrete subclasses; base versions push_error.
- `GdPAIObjectData._init` — auto-adds groups; override get_group_labels/get_provided_actions/get_sim_properties instead.
- `GdPAIBehaviorConfig.apply_to_agent` — extension point is `_populate`.
- All `GdPAIAgent` underscore members and `GdPAIRustBridge._extract_*` — internal.
