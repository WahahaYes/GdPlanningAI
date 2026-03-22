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
	var gdpai_objects: Array[GdPAIObjectData] = []
	for obj: GdPAIObjectData in GdPAIUTILS.get_children_of_type(entity, GdPAIObjectData):
		gdpai_objects.append(obj)
	blackboard.set_property("GDPAI_OBJECTS", gdpai_objects)
	# Apply behavior configurations.
	for behavior_config in config.behavior_configs:
		behavior_config.apply_to_agent(self)
	# Try to find a world node.
	world_node = GdPAIUTILS.get_child_of_type(get_tree().root, GdPAIWorldNode)
	_bridge = GdPAIRustBridge.new()


func _process(delta: float) -> void:
	# Update behavior configurations (property updaters).
	for behavior_config in config.behavior_configs:
		behavior_config.update_properties(self, delta)

	# Until some goals and actions have been provided, this agent is effectively turned off.
	if goals.size() == 0:
		return

	match _planning_strategy:
		GdPAIAgentConfig.PlanningStrategy.CONTINUOUS:
			# Check if a new plan is needed.
			if _current_action_chain.is_empty() or _current_plan_step > _current_action_chain.size():
				await _query_world_state_and_plan()
		GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL:
			# Timer will handle planning only if current plan is finished.
			pass
		GdPAIAgentConfig.PlanningStrategy.ON_DEMAND:
			# Only plan when explicitly requested.
			pass
		GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL_FORCED:
			# Timer will handle planning regardless of current plan status.
			pass

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

	_query_world_state_and_plan()


## Timer callback for interval planning.
func _on_planning_timer_timeout() -> void:
	if goals.size() == 0:
		return
	if _planning_strategy == GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL_FORCED:
		_query_world_state_and_plan()
	elif _planning_strategy == GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL:
		if _current_action_chain.is_empty() or _current_plan_step > _current_action_chain.size():
			_query_world_state_and_plan()
	_planning_timer.start()


## Query world state and initiate planning.
func _query_world_state_and_plan() -> void:
	var worldly_actions: Array[Action] = await _compute_worldly_actions()
	var valid_self_actions: Array[Action] = await _compute_valid_self_actions()
	var all_actions: Array[Action] = []
	all_actions.append_array(valid_self_actions)
	all_actions.append_array(worldly_actions)

	var result: Dictionary = _bridge.build_plan(self, all_actions, goals)
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
			var action_status: Action.Status = action.pre_perform_action(self)
			if action_status == Action.Status.FAILURE:
				# Abort the plan.
				_current_plan_step = action_chain.size()
		# Progress to actions if we passed through the preaction stage.
		if _current_plan_step == -1:
			_current_plan_step += 1

	# Actions.
	if _current_plan_step < action_chain.size():
		var current_action: Action = action_chain[_current_plan_step]
		var action_status: Action.Status = current_action.perform_action(self, delta)
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
			action.post_perform_action(self)
		_current_plan_step += 1


func _compute_valid_self_actions() -> Array[Action]:
	var ws_checkpoint: GdPAIBlackboard = world_node.get_world_state()
	var valid_actions: Array[Action] = []
	for action in self_actions:
		var validity_checks: Array[Precondition] = action.get_validity_checks()
		var is_satisfied: bool = true
		for check: Precondition in validity_checks:
			var status: bool = await check.evaluate(blackboard, ws_checkpoint)
			if not status:
				is_satisfied = false
				break
		if is_satisfied:
			valid_actions.append(action)
	return valid_actions


## Polls all GdPAI objects to get their relevant actions on request.
func _compute_worldly_actions() -> Array[Action]:
	# Refresh the world state.
	var ws_checkpoint: GdPAIBlackboard = world_node.get_world_state()
	var gdpai_objects: Array[GdPAIObjectData] = []
	var raw_objects = ws_checkpoint.get_property("GDPAI_OBJECTS")
	if raw_objects != null:
		gdpai_objects.assign(raw_objects)
		
	var actions: Array[Action] = []
	for gdpai_object: GdPAIObjectData in gdpai_objects:
		var obj_actions: Array[Action] = gdpai_object.get_provided_actions()
		for obj_act: Action in obj_actions:
			# Every action has a set of validity checks which must pass.
			var validity_checks: Array[Precondition] = obj_act.get_validity_checks()
			var is_satisfied: bool = true
			for check: Precondition in validity_checks:
				var status: bool = await check.evaluate(blackboard, ws_checkpoint)
				if not status:
					is_satisfied = false
					break
			if is_satisfied:
				actions.append(obj_act)
	return actions


