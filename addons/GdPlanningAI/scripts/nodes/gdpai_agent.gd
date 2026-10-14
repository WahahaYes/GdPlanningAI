class_name GdPAIAgent
extends Node
## The AI brain of one entity: it plans with the available goals and actions, then executes the
## resulting plan. Add one per AI-controlled entity and assign [member entity] and [member config].[br]
##[br]
## Each frame the agent first runs its [PropertyUpdater]s, then (according to the planning
## strategy in [member GdPAIAgentConfig.planning_strategy]) may submit a planning request, then
## runs the current plan. The loop is:[br]
## 1. The agent gathers its actions ([member self_actions] plus those from world objects'
## [method GdPAIObjectData.get_provided_actions]) and goals, and submits them with snapshots of
## its [member blackboard] and the world state to the background planner. Declarative hooks
## ([method Action.get_preconditions] and friends, [method Goal.compute_reward],
## [method Goal.get_desired_state]) are read at this moment, on the main thread.[br]
## 2. The search runs on a background thread. Its callbacks ([method Action.get_action_cost],
## [method Action.simulate_effect], custom preconditions) are executed on the main thread by the
## plan scheduler during later frames.[br]
## 3. When a plan is found it replaces the current plan. A result submitted before a newer request
## is discarded. A failed search clears the current plan.[br]
## 4. The plan is executed on the main thread: [method Action.pre_perform_action] for every
## action once, then [method Action.perform_action] every frame for the current step until it
## returns [constant Action.Status.SUCCESS] (next step) or [constant Action.Status.FAILURE]
## (abort), and finally [method Action.post_perform_action] for every action once when the plan
## ends, whether it completed or was aborted.[br]
##[br]
## The agent needs a [GdPAIWorldNode] in the scene and at least one goal to do anything. Users
## normally configure it entirely through the inspector and [GdPAIBehaviorConfig]s.

## The top-level node of the entity this agent controls. Object data nodes below it
## ([GdPAIObjectData], [GdPAILocationData], ...) become part of the agent's own simulated state, and
## [GoToAction] searches it for a navigation agent. Required for planning.
@export var entity: Node
## Planning settings, starting blackboard and behaviors. Read once when the agent becomes ready.
@export var config: GdPAIAgentConfig = GdPAIAgentConfig.new()

## The agent's own blackboard, created in [code]_ready[/code] from
## [member GdPAIAgentConfig.blackboard_plan]. Holds the agent's properties (hunger, held items, ...),
## the [code]entity[/code] reference and per-action [method Action.set_state] data.
var blackboard: GdPAIBlackboard
## The [GdPAIWorldNode] found in the scene when the agent became ready, or [code]null[/code]. The
## agent cannot plan without it.
var world_node: GdPAIWorldNode = null
## The goals this agent pursues, filled by its behavior configs. The agent is idle while empty.
var goals: Array[Goal]
## Actions this agent owns, filled by its behavior configs. Actions offered by world objects are
## gathered separately at each plan request.
var self_actions: Array[Action]
## Property updaters registered by behavior configs. Each is updated every frame, before
## planning and plan execution.
var property_updaters: Array[PropertyUpdater]
## The currently selected goal.
var _current_goal: Goal = null
## The Rust planning bridge.
var _bridge: GdPAIRustBridge
## The current action chain being executed.
var _current_action_chain: Array[Action] = []
## Bindings for each chain position, keyed by position.
var _current_bindings_by_position: Dictionary = {}
## The current step within a plan.  Also used to control flow for pre/post actions.
var _current_plan_step: int = -1
## Planning strategy for this agent.
var _planning_strategy: GdPAIAgentConfig.PlanningStrategy = (
	GdPAIAgentConfig.PlanningStrategy.CONTINUOUS
)
## Timer for interval-based planning.
var _planning_timer: Timer = null
## True while a background plan is in flight.
var _waiting_for_plan: bool = false
## Actions submitted with the last async plan request (needed for deserialization).
var _last_submitted_actions: Array[Action] = []
## Incremented on each async submit; checked in _on_plan_ready to discard stale results.
var _plan_generation: int = 0
## The generation that the in-flight plan was submitted under.
var _inflight_generation: int = 0


## Applies the planning strategy, creates the blackboard, registers the entity's object data,
## applies every behavior config and looks up the [GdPAIWorldNode].
func _ready() -> void:
	# Set planning strategy.
	set_planning_strategy(config.planning_strategy, config.planning_interval)

	blackboard = config.blackboard_plan.generate_blackboard()
	# Initial blackboard setup common for all agents.
	blackboard.set_property("entity", entity)
	# Collect any GdPAI nodes under this agent's entity.
	if entity != null:
		var agent_objects: Array = GdPAIUTILS.get_children_in_group(entity, "GdPAIObjectData")
		blackboard.set_property("GDPAI_OBJECTS", agent_objects)
	# Apply behavior configurations.
	for behavior_config in config.behavior_configs:
		behavior_config.apply_to_agent(self)
	# Try to find a world node.
	world_node = GdPAIUTILS.get_child_of_type(get_tree().root, GdPAIWorldNode)
	_bridge = GdPAIRustBridge.new()


## Per-frame loop: runs property updaters, starts a plan when the strategy asks for one, then
## executes the current plan.
func _process(delta: float) -> void:
	for updater in property_updaters:
		updater.update_properties(self, delta)

	# Until some goals and actions have been provided, this agent is effectively turned off.
	if goals.size() == 0:
		return

	if _planning_strategy == GdPAIAgentConfig.PlanningStrategy.CONTINUOUS:
		# Check if a new plan is needed.
		var plan_done: bool = (
			_current_action_chain.is_empty() or _current_plan_step > _current_action_chain.size()
		)
		if plan_done and not _waiting_for_plan:
			_start_plan_async()

	_execute_plan(delta)


## Returns the goal the current plan serves, or [code]null[/code] if there is no plan.
func get_current_goal() -> Goal:
	return _current_goal


## Returns the action chain of the current plan in execution order, or an empty array. The
## actions are the per-plan instances from [method Action.clone_for_plan].
func get_current_plan() -> Array[Action]:
	return _current_action_chain


## Returns the index of the step being executed. [code]-1[/code] means a new plan is waiting for
## its [method Action.pre_perform_action] phase; a value of at least the chain size means the plan
## has ended.
func get_current_plan_step() -> int:
	return _current_plan_step


## Changes the planning strategy at runtime. [param interval] is the seconds between checks for
## [constant GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL] and
## [constant GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL_FORCED]; it is ignored otherwise. The
## first interval is randomised to spread agents out. Any previous timer is replaced.
func set_planning_strategy(
	strategy: GdPAIAgentConfig.PlanningStrategy,
	interval: float = 0.5,
) -> void:
	_planning_strategy = strategy

	if _planning_timer:
		# Stop first: a queue_freed timer lives until the end of the frame and could still fire.
		_planning_timer.stop()
		_planning_timer.queue_free()
		_planning_timer = null

	if (
		strategy
		in [
			GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL,
			GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL_FORCED,
		]
	):
		_planning_timer = Timer.new()
		_planning_timer.timeout.connect(_on_planning_timer_timeout)
		add_child(_planning_timer)

		# Add randomized delay before starting planning
		var random_delay: float = randf_range(0.0, interval)
		_planning_timer.start(random_delay)
		_planning_timer.wait_time = interval


## Submits a planning request now, regardless of strategy and of any plan currently running.
## Does nothing while the agent has no goals. When the resulting plan is ready it replaces the
## current one; the old plan's [method Action.post_perform_action] is not called in that case.
func manually_start_plan() -> void:
	if goals.size() == 0:
		return

	_start_plan_async()


## Submit a planning job to the background scheduler.
func _start_plan_async() -> void:
	var agent_objects: Array = GdPAIUTILS.get_children_in_group(entity, "GdPAIObjectData")
	blackboard.set_property("GDPAI_OBJECTS", agent_objects)

	var all_actions: Array[Action] = []
	all_actions.append_array(self_actions)
	all_actions.append_array(_collect_worldly_actions())

	_last_submitted_actions = all_actions
	_plan_generation += 1
	_inflight_generation = _plan_generation
	_waiting_for_plan = true

	var scheduler: GdPAIPlanScheduler = GdPAIAutoload.get_scheduler()
	if scheduler == null:
		_waiting_for_plan = false
		push_error("GdPAIAgent: scheduler not available; async planning is required")
		return

	if world_node == null:
		_waiting_for_plan = false
		push_warning("GdPAIAgent: no GdPAIWorldNode found in scene")
		return

	(
		scheduler
		. submit_plan(
			self,
			blackboard,
			world_node.get_world_state(),
			_bridge.serialize_actions(all_actions),
			_bridge.serialize_goals(goals, self),
			config.max_recursion,
			config.iteration_budget,
			config.time_slice_ms,
		)
	)


## Called by the scheduler on the main thread when a background plan completes. Installs the
## new plan (resetting the step to -1) or clears the current plan when the search failed.
func _on_plan_ready(result: Dictionary) -> void:
	_waiting_for_plan = false
	# Discard stale results if goals changed since submission.
	if _inflight_generation != _plan_generation:
		return
	_current_plan_step = -1
	if result.get("success", false):
		var deserialized: Dictionary = _bridge.deserialize_plan_result(
			result, _last_submitted_actions
		)
		_current_action_chain = deserialized.get("action_chain", [] as Array[Action])
		_current_bindings_by_position = deserialized.get("bindings_by_position", {})
		var goal_index: int = result.get("goal_index", 0)
		if goal_index >= 0 and goal_index < goals.size():
			_current_goal = goals[goal_index]
		else:
			push_error(
				(
					"GdPAIAgent: goal_index %d out of bounds (goals.size()=%d)"
					% [goal_index, goals.size()]
				)
			)
			_current_goal = null
	else:
		_current_action_chain = []
		_current_bindings_by_position = {}
		_current_goal = null


## Timer callback for interval planning.
func _on_planning_timer_timeout() -> void:
	# A timer replaced by set_planning_strategy can still deliver a timeout; ignore it.
	if _planning_timer == null:
		return
	if goals.size() == 0:
		return
	if _planning_strategy == GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL_FORCED:
		_start_plan_async()
	elif _planning_strategy == GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL:
		var plan_done: bool = (
			_current_action_chain.is_empty() or _current_plan_step > _current_action_chain.size()
		)
		if plan_done and not _waiting_for_plan:
			_start_plan_async()
	_planning_timer.start()


## Injects the planner bindings for a specific chain position into the action instance.
func _inject_bindings_for_position(chain_position: int, action: Action) -> void:
	if not _current_bindings_by_position.has(chain_position):
		return
	for binding in _current_bindings_by_position[chain_position]:
		var fact_name: String = binding[1]
		var values: Array = binding[2]
		var object_refs = []
		for val in values:
			if val is int:
				var obj = instance_from_id(val)
				if obj != null:
					object_refs.append(obj)
			elif val != null:
				# Already an object or other variant
				object_refs.append(val)
		if action.has_method("inject_binding"):
			action.inject_binding(fact_name, object_refs)


## Executes the currently selected plan based on the current step.
func _execute_plan(delta: float) -> void:
	if _current_action_chain.is_empty():
		return
	var action_chain: Array[Action] = _current_action_chain
	# Pre actions.
	if _current_plan_step == -1:
		for i in range(action_chain.size()):
			var action: Action = action_chain[i]
			if not is_instance_valid(action):
				_current_plan_step = action_chain.size()
				break
			action.chain_position = i
			_inject_bindings_for_position(i, action)
			var action_status: Action.Status = action.pre_perform_action(self)
			if action_status == Action.Status.FAILURE:
				# Abort the plan; post_perform_action will still be called for cleanup.
				_current_plan_step = action_chain.size()
				break
		# Progress to actions if we passed through the preaction stage.
		if _current_plan_step == -1:
			_current_plan_step += 1

	# Actions.
	if _current_plan_step < action_chain.size():
		var current_action: Action = action_chain[_current_plan_step]
		if not is_instance_valid(current_action):
			_current_plan_step = action_chain.size()
		else:
			current_action.chain_position = _current_plan_step
			_inject_bindings_for_position(_current_plan_step, current_action)
			var action_status: Action.Status = current_action.perform_action(self, delta)
			if action_status == Action.Status.FAILURE:
				# Abort the plan; post_perform_action will still be called for cleanup.
				_current_plan_step = action_chain.size()
			elif action_status == Action.Status.RUNNING:
				# Continue performing this action.
				pass
			elif action_status == Action.Status.SUCCESS:
				# Progress to the next action.
				_current_plan_step += 1

	# Post actions.
	# This is a guaranteed cleanup phase: post_perform_action is called for every action
	# in the chain, regardless of whether the plan completed or was aborted. Implementations
	# must be safe to call even if pre_perform_action or perform_action returned FAILURE.
	if _current_plan_step == action_chain.size():
		for i in range(action_chain.size()):
			var action: Action = action_chain[i]
			if is_instance_valid(action):
				action.chain_position = i
				_inject_bindings_for_position(i, action)
				action.post_perform_action(self)
				action.chain_position = -1
		_current_plan_step += 1
		_current_bindings_by_position = {}


## Collects actions provided by world objects. Validity filtering is handled
## by the Rust engine during planning search.
func _collect_worldly_actions() -> Array[Action]:
	if world_node == null:
		return [] as Array[Action]
	var ws: GdPAIBlackboard = world_node.get_world_state()
	var actions: Array[Action] = []
	var raw_objects = ws.get_property("GDPAI_OBJECTS")
	if raw_objects == null:
		return actions
	for gdpai_object: GdPAIObjectData in raw_objects:
		actions.append_array(gdpai_object.get_provided_actions())
	return actions
