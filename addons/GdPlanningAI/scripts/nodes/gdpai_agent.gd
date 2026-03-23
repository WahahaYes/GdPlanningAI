class_name GdPAIAgent
extends Node
## GdPlanningAI Agent.  The agent has their own record of the world state and a
## personal blackboard of attributes.  Agents form plans given their available goals and actions.

## The top-level node of the agent.
@export var entity: Node
## Configuration resource for agent setup.
@export var config: GdPAIAgentConfig = GdPAIAgentConfig.new()

## Reference to this agent's own blackboard.
var blackboard: GdPAIBlackboard
## Reference to the world node.
var world_node: GdPAIWorldNode = null
## List of this agent's goals.
var goals: Array[Goal]
## List of this agent's available actions.
var self_actions: Array[Action]
## Property updaters registered by behavior configs. Updated every frame.
var property_updaters: Array[PropertyUpdater]
## The currently selected goal.
var _current_goal: Goal = null
## The Rust planning bridge.
var _bridge: GdPAIRustBridge
## The current action chain being executed.
var _current_action_chain: Array[Action] = []
## The current step within a plan.  Also used to control flow for pre/post actions.
var _current_plan_step: int = -1
## Planning strategy for this agent.
var _planning_strategy: GdPAIAgentConfig.PlanningStrategy = \
GdPAIAgentConfig.PlanningStrategy.CONTINUOUS
## Timer for interval-based planning.
var _planning_timer: Timer = null


func _ready() -> void:
	# Set planning strategy.
	set_planning_strategy(config.planning_strategy, config.planning_interval)

	blackboard = config.blackboard_plan.generate_blackboard()
	# Initial blackboard setup common for all agents.
	blackboard.set_property("entity", entity)
	# Collect any GdPAI nodes under this agent's entity.
	var agent_objects: Array = GdPAIUTILS.get_children_in_group(entity, "GdPAIObjectData")
	blackboard.set_property("GDPAI_OBJECTS", agent_objects)
	# Apply behavior configurations.
	for behavior_config in config.behavior_configs:
		behavior_config.apply_to_agent(self )
	# Try to find a world node.
	world_node = GdPAIUTILS.get_child_of_type(get_tree().root, GdPAIWorldNode)
	_bridge = GdPAIRustBridge.new()
	_bridge.planning_engine.set_max_recursion(config.max_recursion)


func _process(delta: float) -> void:
	for updater in property_updaters:
		updater.update_properties(self , delta)

	# Until some goals and actions have been provided, this agent is effectively turned off.
	if goals.size() == 0:
		return

	if _planning_strategy == GdPAIAgentConfig.PlanningStrategy.CONTINUOUS:
		# Check if a new plan is needed.
		var plan_done: bool = (
			_current_action_chain.is_empty()
			or _current_plan_step > _current_action_chain.size()
		)
		if plan_done:
			_start_plan()

	_execute_plan(delta)


## Returns the currently selected goal.
func get_current_goal() -> Goal:
	return _current_goal


## Returns the current action chain being executed.
func get_current_plan() -> Array[Action]:
	return _current_action_chain


## Returns the current step within the plan.
func get_current_plan_step() -> int:
	return _current_plan_step


## Set the planning strategy for this agent.
func set_planning_strategy(
		strategy: GdPAIAgentConfig.PlanningStrategy,
		interval: float = 0.5,
) -> void:
	_planning_strategy = strategy

	if _planning_timer:
		_planning_timer.queue_free()
		_planning_timer = null

	if (
		strategy in [
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


## Trigger planning on demand.
func manually_start_plan() -> void:
	if goals.size() == 0:
		return

	_start_plan()


## Timer callback for interval planning.
func _on_planning_timer_timeout() -> void:
	if goals.size() == 0:
		return
	if _planning_strategy == GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL_FORCED:
		_start_plan()
	elif _planning_strategy == GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL:
		var plan_done: bool = (
			_current_action_chain.is_empty()
			or _current_plan_step > _current_action_chain.size()
		)
		if plan_done:
			_start_plan()
	_planning_timer.start()


## Collects all candidate actions and asks the Rust engine for a plan.
## Fully synchronous — no await.
func _start_plan() -> void:
	# Refresh the agent's own object snapshots so proxy positions are current.
	var agent_objects: Array = GdPAIUTILS.get_children_in_group(entity, "GdPAIObjectData")
	blackboard.set_property("GDPAI_OBJECTS", agent_objects)

	var all_actions: Array[Action] = []
	all_actions.append_array(self_actions)
	all_actions.append_array(_collect_worldly_actions())

	var result: Dictionary = _bridge.build_plan(
		blackboard,
		world_node.get_world_state(),
		all_actions,
		goals,
		self ,
	)
	_current_plan_step = -1
	if result.get("success", false):
		_current_action_chain = _bridge.deserialize_plan_result(result, all_actions)
		_current_goal = goals[result.get("goal_index", 0)]
	else:
		_current_action_chain = []
		_current_goal = null


## Executes the currently selected plan based on the current step.
func _execute_plan(delta: float) -> void:
	if _current_action_chain.is_empty():
		return
	var action_chain: Array[Action] = _current_action_chain
	# Pre actions.
	if _current_plan_step == -1:
		for action: Action in action_chain:
			var action_status: Action.Status = action.pre_perform_action(self )
			if action_status == Action.Status.FAILURE:
				# Abort the plan.
				_current_plan_step = action_chain.size()
		# Progress to actions if we passed through the preaction stage.
		if _current_plan_step == -1:
			_current_plan_step += 1

	# Actions.
	if _current_plan_step < action_chain.size():
		var current_action: Action = action_chain[_current_plan_step]
		var action_status: Action.Status = current_action.perform_action(self , delta)
		if action_status == Action.Status.FAILURE:
			# Abort the plan.
			_current_plan_step = action_chain.size()
		elif action_status == Action.Status.RUNNING:
			# Continue performing this action.
			pass
		elif action_status == Action.Status.SUCCESS:
			# Progress to the next action.
			_current_plan_step += 1

	# Post actions.
	if _current_plan_step == action_chain.size(): # We just finished, do post actions.
		for action: Action in action_chain:
			action.post_perform_action(self )
		_current_plan_step += 1


## Collects actions provided by world objects. Validity filtering is handled
## by the Rust engine during planning search.
func _collect_worldly_actions() -> Array[Action]:
	var ws: GdPAIBlackboard = world_node.get_world_state()
	var actions: Array[Action] = []
	var raw_objects = ws.get_property("GDPAI_OBJECTS")
	if raw_objects == null:
		return actions
	for gdpai_object: GdPAIObjectData in raw_objects:
		actions.append_array(gdpai_object.get_provided_actions())
	return actions
