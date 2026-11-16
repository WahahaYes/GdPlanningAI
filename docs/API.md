# API

The public surface of GdPlanningAI: every class, member and signature users can reach. Generated from the code by `scripts/api_surface.py` (`make api`); a change to the surface shows here as a diff, and necro fails while this file is stale. What each member does is in its docstring and [AUTHORING_GUIDE.md](AUTHORING_GUIDE.md).

## GDScript

### Action

`addons/GdPlanningAI/scripts/refcounteds/action.gd`

```gdscript
extends RefCounted
enum Status { FAILURE, RUNNING, SUCCESS }
var chain_position: int
func get_validity_checks() -> Array[Precondition]
func get_cost(_agent_blackboard: GdPAIBlackboard, _world_state: GdPAIBlackboard) -> float
func get_preconditions() -> Array[Precondition]
func get_requirements() -> Array[RequirementSpec]
func get_provisions() -> Array[ProvisionSpec]
func inject_binding(_fact_name: String, _value: Array) -> void
func simulate_effect(_agent_blackboard: GdPAIBlackboard, _world_state: GdPAIBlackboard) -> void
func get_effect_spec() -> Array
func pre_perform_action(_agent: GdPAIAgent) -> Status
func perform_action(_agent: GdPAIAgent, _delta: float) -> Status
func post_perform_action(_agent: GdPAIAgent) -> Status
func set_state(agent: GdPAIAgent, key: String, value: Variant) -> void
func get_state(agent: GdPAIAgent, key: String) -> Variant
func erase_state(agent: GdPAIAgent, key: String) -> void
func has_state(agent: GdPAIAgent, key: String) -> bool
func erase_position_state(agent: GdPAIAgent, pos: int) -> void
func erase_all_state(agent: GdPAIAgent) -> void
func clone_for_plan() -> Action
func get_title() -> String
func get_description() -> String
```

### GdPAIAgent

`addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd`

```gdscript
extends Node
signal plan_delivered(success: bool, goal: Goal, plan: Array[Action], cost: float)
const PRE_PHASE: int = -1
const KEY_ENTITY: String = "entity"
const KEY_OBJECTS: String = "GDPAI_OBJECTS"
@export var entity: Node
@export var config: GdPAIAgentConfig = GdPAIAgentConfig.new()
var blackboard: GdPAIBlackboard
var world_node: GdPAIWorldNode
var goals: Array[Goal]
var self_actions: Array[Action]
var property_updaters: Array[PropertyUpdater]
func get_current_goal() -> Goal
func get_current_plan() -> Array[Action]
func get_current_plan_step() -> int
func get_current_action() -> Action
func is_waiting_for_plan() -> bool
func set_planning_strategy(strategy: GdPAIAgentConfig.PlanningStrategy, interval: float = 0.5) -> void
func request_plan() -> void
```

### GdPAIAgentConfig

`addons/GdPlanningAI/scripts/resources/gdpai_agent_config.gd`

```gdscript
extends Resource
enum PlanningStrategy { CONTINUOUS, ON_INTERVAL, ON_DEMAND, ON_INTERVAL_FORCED }
@export var planning_strategy: PlanningStrategy = PlanningStrategy.CONTINUOUS
@export var planning_interval: float = 0.5
@export var failure_backoff: float = 0.0
@export var max_recursion: int = 100
@export var iteration_budget: int = 20000
@export var time_slice_ms: int = 10
@export var max_effect_fallbacks: int = 128
@export var blackboard_plan: GdPAIBlackboardPlan = GdPAIBlackboardPlan.new()
@export var behavior_configs: Array[GdPAIBehaviorConfig] = []
```

### GdPAIAutoload

`addons/GdPlanningAI/gdpai_autoload.gd`

```gdscript
extends Node
func get_scheduler() -> GdPAIPlanScheduler
```

### GdPAIBehaviorConfig

`addons/GdPlanningAI/scripts/resources/gdpai_behavior_config.gd`

```gdscript
extends Resource
func apply_to_agent(agent: GdPAIAgent) -> void
```

### GdPAIBlackboardPlan

`addons/GdPlanningAI/scripts/gdpai_blackboard_plan.gd`

```gdscript
extends Resource
@export var blackboard_backend: Dictionary
func generate_blackboard() -> GdPAIBlackboard
```

### GdPAIInteractable

`addons/GdPlanningAI/scripts/nodes/gdpai_interactable.gd`

```gdscript
extends GdPAIObjectData
@export var max_interaction_distance: float = 2
@export var max_drift_from_plan: float = -1
func get_group_labels() -> Array[String]
func get_sim_properties() -> Dictionary
```

### GdPAILocationData

`addons/GdPlanningAI/scripts/nodes/gdpai_location_data.gd`

```gdscript
extends GdPAIObjectData
@export var location_node_2d: Node2D
@export var location_node_3d: Node3D
var position
var rotation
func get_group_labels() -> Array[String]
func get_sim_properties() -> Dictionary
```

### GdPAIObjectData

`addons/GdPlanningAI/scripts/nodes/gdpai_object_data.gd`

```gdscript
extends Node
@export var entity: Node
func get_group_labels() -> Array[String]
func get_provided_actions() -> Array[Action]
func get_sim_properties() -> Dictionary
```

### GdPAIRustBridge

`addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd`

```gdscript
extends RefCounted
func deserialize_plan_result(result: Dictionary, actions: Array[Action]) -> Dictionary
func serialize_actions(actions: Array[Action]) -> Array[Dictionary]
func serialize_goals(goals: Array[Goal], agent: GdPAIAgent) -> Array[Dictionary]
```

### GdPAIUtils

`addons/GdPlanningAI/gdpai_utils.gd`

```gdscript
extends Object
static func get_child_of_type(node: Node, _class: Variant) -> Variant
static func get_children_in_group(node: Node, group: String) -> Array
```

### GdPAIWorldNode

`addons/GdPlanningAI/scripts/nodes/gdpai_world_node.gd`

```gdscript
extends Node
@export var blackboard_plan: GdPAIBlackboardPlan
@onready var world_state: GdPAIBlackboard
func get_world_state() -> GdPAIBlackboard
```

### Goal

`addons/GdPlanningAI/scripts/refcounteds/goal.gd`

```gdscript
extends RefCounted
func get_reward(_agent: GdPAIAgent) -> float
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]
func get_title() -> String
func get_description() -> String
```

### GoToAction

`addons/GdPlanningAI/scripts/refcounteds/goto_action.gd`

```gdscript
extends Action
const ARRIVAL_THRESHOLD_2D: float = 8.0
const ARRIVAL_THRESHOLD_3D: float = 0.1
const STUCK_WINDOW: float = 1.0
const PROGRESS_WINDOW: float = 3.0
const STUCK_ARRIVAL_FACTOR: float = 2.0
var target_location: GdPAILocationData
static func find_nav_agent(entity: Node) -> Node
func _init(p_target_location: GdPAILocationData = null) -> void
func clone_for_plan() -> Action
func inject_binding(fact_name: String, value: Array) -> void
func get_cost(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> float
func get_validity_checks() -> Array[Precondition]
func get_provisions() -> Array[ProvisionSpec]
func get_requirements() -> Array[RequirementSpec]
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> void
func get_effect_spec() -> Array
func pre_perform_action(agent: GdPAIAgent) -> Action.Status
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status
func post_perform_action(agent: GdPAIAgent) -> Action.Status
func get_title() -> String
func get_description() -> String
```

### Precondition

`addons/GdPlanningAI/scripts/refcounteds/precondition.gd`

```gdscript
extends RefCounted
func to_bridge_dict() -> Dictionary
static func custom(fn: Callable) -> Precondition
static func custom_with_deps(fn: Callable, deps: Array[Object]) -> Precondition
static func agent_has_property(prop: String) -> Precondition
static func agent_property_not_equal_to(prop: String, value: Variant) -> Precondition
static func agent_property_greater_than(prop: String, value: Variant) -> Precondition
static func agent_property_at_least(prop: String, value: Variant) -> Precondition
static func agent_property_less_than(prop: String, value: Variant) -> Precondition
static func agent_property_at_most(prop: String, value: Variant) -> Precondition
static func agent_property_equal_to(prop: String, value: Variant) -> Precondition
static func world_state_has_property(prop: String) -> Precondition
static func world_state_property_greater_than(prop: String, value: Variant) -> Precondition
static func world_state_property_at_least(prop: String, value: Variant) -> Precondition
static func world_state_property_less_than(prop: String, value: Variant) -> Precondition
static func world_state_property_at_most(prop: String, value: Variant) -> Precondition
static func world_state_property_equal_to(prop: String, value: Variant) -> Precondition
static func world_state_property_not_equal_to(prop: String, value: Variant) -> Precondition
static func world_object_property_equal_to(group: String, property: String, value: Variant) -> Precondition
static func world_object_property_not_equal_to(group: String, property: String, value: Variant) -> Precondition
static func world_object_property_greater_than(group: String, property: String, value: Variant) -> Precondition
static func world_object_property_at_least(group: String, property: String, value: Variant) -> Precondition
static func world_object_property_less_than(group: String, property: String, value: Variant) -> Precondition
static func world_object_property_at_most(group: String, property: String, value: Variant) -> Precondition
static func world_object_has_property(group: String, property: String) -> Precondition
static func agent_has_object_data_of_group(group: String) -> Precondition
static func world_state_has_object_data_of_group(group: String) -> Precondition
static func object_is_valid(object: Variant) -> Precondition
```

### PreconditionBuiltin

`addons/GdPlanningAI/scripts/refcounteds/precondition_builtin.gd`

```gdscript
extends Precondition
enum Target { AGENT, WORLD_STATE, WORLD_OBJECT_PROXY }
enum Op { HAS_PROPERTY, EQUAL, NOT_EQUAL, GT, GTE, LT, LTE }
var target: Target
var operation: Op
var property: String
var group: String
var value: Variant
func _init(t: Target, op: Op, prop: String, val: Variant = null, grp: String = "") -> void
func to_bridge_dict() -> Dictionary
```

### PreconditionCustom

`addons/GdPlanningAI/scripts/refcounteds/precondition_custom.gd`

```gdscript
extends Precondition
var eval_func: Callable
func _init(fn: Callable) -> void
func to_bridge_dict() -> Dictionary
```

### PreconditionCustomWithDeps

`addons/GdPlanningAI/scripts/refcounteds/precondition_custom_with_deps.gd`

```gdscript
extends Precondition
var eval_func: Callable
var dependent_objects: Array[Object]
func _init(fn: Callable, deps: Array[Object] = []) -> void
func to_bridge_dict() -> Dictionary
```

### PropertyUpdater

`addons/GdPlanningAI/scripts/refcounteds/property_updater.gd`

```gdscript
extends RefCounted
func update_properties(_agent: GdPAIAgent, _delta: float) -> void
func initialize(_agent: GdPAIAgent) -> void
```

### ProvisionSpec

`addons/GdPlanningAI/scripts/refcounteds/provision_spec.gd`

```gdscript
extends RefCounted
func to_bridge_dict() -> Dictionary
static func binding(name: String, value: Variant) -> ProvisionSpec
static func fact(name: String, args: Array[Variant] = []) -> ProvisionSpec
static func fact_wildcard(name: String) -> ProvisionSpec
```

### ProvisionSpecBinding

`addons/GdPlanningAI/scripts/refcounteds/provision_spec_binding.gd`

```gdscript
extends ProvisionSpec
var binding_name: String
var value: Variant
func _init(name: String, val: Variant) -> void
func to_bridge_dict() -> Dictionary
```

### ProvisionSpecFact

`addons/GdPlanningAI/scripts/refcounteds/provision_spec_fact.gd`

```gdscript
extends ProvisionSpec
var fact_name: String
var args: Array[Variant]
func _init(name: String, fact_args: Array[Variant] = []) -> void
func to_bridge_dict() -> Dictionary
```

### ProvisionSpecFactWildcard

`addons/GdPlanningAI/scripts/refcounteds/provision_spec_fact_wildcard.gd`

```gdscript
extends ProvisionSpec
var fact_name: String
func _init(name: String) -> void
func to_bridge_dict() -> Dictionary
```

### RequirementSpec

`addons/GdPlanningAI/scripts/refcounteds/requirement_spec.gd`

```gdscript
extends RefCounted
func to_bridge_dict() -> Dictionary
static func binding_exists(name: String) -> RequirementSpec
static func binding_equals(name: String, value: Variant) -> RequirementSpec
static func binding_in_set(name: String, set_name: String) -> RequirementSpec
static func fact(name: String, args: Array[Variant] = []) -> RequirementSpec
```

### RequirementSpecBindingEquals

`addons/GdPlanningAI/scripts/refcounteds/requirement_spec_binding_equals.gd`

```gdscript
extends RequirementSpec
var binding_name: String
var value: Variant
func _init(name: String, val: Variant) -> void
func to_bridge_dict() -> Dictionary
```

### RequirementSpecBindingExists

`addons/GdPlanningAI/scripts/refcounteds/requirement_spec_binding_exists.gd`

```gdscript
extends RequirementSpec
var binding_name: String
func _init(name: String) -> void
func to_bridge_dict() -> Dictionary
```

### RequirementSpecBindingInSet

`addons/GdPlanningAI/scripts/refcounteds/requirement_spec_binding_in_set.gd`

```gdscript
extends RequirementSpec
var binding_name: String
var set_name: String
func _init(name: String, target_set_name: String) -> void
func to_bridge_dict() -> Dictionary
```

### RequirementSpecFact

`addons/GdPlanningAI/scripts/refcounteds/requirement_spec_fact.gd`

```gdscript
extends RequirementSpec
var fact_name: String
var args: Array[Variant]
func _init(name: String, fact_args: Array[Variant] = []) -> void
func to_bridge_dict() -> Dictionary
```

## Rust (GDExtension)

### GdPAIBlackboard

```rust
extends RefCounted
fn get_property(key: GString) -> Variant
fn set_property(key: GString, value: Variant)
fn has_property(key: GString) -> bool
fn erase_property(key: GString)
fn get_dict() -> VarDictionary
fn set_dict(dict: VarDictionary)
fn get_proxies_in_group(group: GString) -> Array<Gd<SimObjectProxy>>
fn get_proxy_in_group(group: GString) -> Variant
fn get_node_in_group(group: GString) -> Variant
fn get_nodes_in_group(group: GString) -> Array<Variant>
fn get_object_for(node_or_id: Variant) -> Variant
fn clone_for_simulation() -> Gd<GdPAIBlackboard>
```

### GdPAIPlanScheduler

```rust
extends Node
@export var max_threads: i64
fn process_callbacks()
fn submit_plan(agent: Gd<Object>, agent_bb: Gd<GdPAIBlackboard>, world_bb: Gd<GdPAIBlackboard>, actions: Array<VarDictionary>, goals: Array<VarDictionary>, max_recursion: i64, iteration_budget: i64, time_slice_ms: i64, max_effect_fallbacks: i64)
fn active_job_count() -> i64
fn cancel_agent_jobs(agent: Gd<Object>)
fn clear_active_jobs()
fn cancel_all_jobs()
fn get_pool_status() -> String
fn get_effect_dispatch_stats() -> VarDictionary
fn get_engine_version() -> String
fn set_log_level(level: i64)
fn get_debug_tree(agent: Gd<Object>) -> String
```

### SimObjectProxy

```rust
extends RefCounted
fn is_in_group(group: GString) -> bool
fn get_groups() -> Array<GString>
fn get_property(key: GString) -> Variant
fn set_property(key: GString, value: Variant)
fn has_property(key: GString) -> bool
```
