class_name GdPAIAgent
extends Node
## GdPlanningAI Agent.  The agent has their own record of the world state and a
## personal blackboard of attributes.  Agents form plans given their available goals and actions.

## The top-level node of the agent.
@export var entity: Node
## Configuration resource for agent setup.
@export var config: GdPAIAgentConfig = GdPAIAgentConfig.new()

## Agent has ownership over one thread during its lifespan.
var thread: Thread = null
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
## The current plan being followed.
var _current_plan: Plan = null
## The current step within a plan.  Also used to control flow for pre/post actions.
var _current_plan_step: int = -1
## An array for tracking the runtime status of each node in the plan.
var _runtime_status: Dictionary = { }
## Planning strategy for this agent.
var _planning_strategy: GdPAIAgentConfig.PlanningStrategy = \
GdPAIAgentConfig.PlanningStrategy.CONTINUOUS
## Timer for interval-based planning.
var _planning_timer: Timer = null


func _ready() -> void:
	# Set planning strategy.
	set_planning_strategy(config.planning_strategy, config.planning_interval)

	if config.use_multithreading:
		thread = Thread.new()
	blackboard = config.blackboard_plan.generate_blackboard()
	# Initial blackboard setup common for all agents.
	blackboard.set_property("entity", entity)
	# Collect any GdPAI nodes under this agent's entity.
	var gdpai_objects: Array[GdPAIObjectData] = []
	for obj: GdPAIObjectData in GdPAIUTILS.get_children_of_type(entity, GdPAIObjectData):
		gdpai_objects.append(obj)
	blackboard.set_property(GdPAIBlackboard.GDPAI_OBJECTS, gdpai_objects)
	# Apply behavior configurations.
	for behavior_config in config.behavior_configs:
		behavior_config.apply_to_agent(self)
	# Try to find a world node.
	world_node = GdPAIUTILS.get_child_of_type(get_tree().root, GdPAIWorldNode)
	# Notify debugger of agent creation.
	EngineDebugger.send_message(
		"gdplanningai:register_agent",
		[get_instance_id(), "%s (%s)" % [name, get_instance_id()]],
	)


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
			if _current_plan == null or _current_plan_step > _current_plan.get_plan().size():
				await _query_world_state_and_plan()
		GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL:
			# Timer will handle planning.
			pass
		GdPAIAgentConfig.PlanningStrategy.ON_DEMAND:
			# Only plan when explicitly requested.
			pass

	_execute_plan(delta)


## Returns the currently selected goal.
func get_current_goal() -> Goal:
	return _current_goal


## Returns the current plan being followed.
func get_current_plan() -> Plan:
	return _current_plan


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

	if strategy == GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL:
		_planning_timer = Timer.new()
		_planning_timer.wait_time = interval
		_planning_timer.timeout.connect(_on_planning_timer_timeout)
		add_child(_planning_timer)
		_planning_timer.autostart = true


## Trigger planning on demand.
func manually_start_plan() -> void:
	if goals.size() == 0:
		return

	_query_world_state_and_plan()


## Timer callback for interval planning.
func _on_planning_timer_timeout() -> void:
	if goals.size() == 0:
		return

	_query_world_state_and_plan()


## Query world state and initiate planning.
func _query_world_state_and_plan() -> void:
	# Query the world state at the start of planning.
	var worldly_actions: Array[Action] = await _compute_worldly_actions()
	var self_actions: Array[Action] = await _compute_valid_self_actions()
	if config.use_multithreading:
		# Spin up a thread to call the planning logic.  Variable assignment happens at the
		# end of planning.
		if not thread.is_started():
			# Spin up a new thread to handle planning.
			_current_plan = null
			_current_plan_step = -1
			thread.start(
				_select_highest_reward_goal.bind(self_actions, worldly_actions),
				config.thread_priority,
			)
	else:
		var goal_and_plan: Dictionary = await _select_highest_reward_goal(
			self_actions,
			worldly_actions,
		)
		_current_plan_step = -1
		_current_goal = goal_and_plan["goal"]
		_current_plan = goal_and_plan["plan"]
		if _current_plan != null:
			_reset_runtime_status(_current_plan.get_plan())
			_update_debugger_info()


## Iterates over all goals in order of reward until a valid plan is found.
func _select_highest_reward_goal(
		self_actions: Array[Action],
		worldly_actions: Array[Action],
) -> Dictionary:
	if config.use_multithreading:
		# Removing safety checks usually isn't a good idea.  In our processing we will read from
		# the scene tree or will await information, but we don't write.  With some checks to
		# ensure that data exists, and knowing that this thread's processing is constrained to
		# planning, this is maybe okay to do.
		thread.set_thread_safety_checks_enabled(false)

	var return_dict: Dictionary = { }
	var rewards: Array[float] = []

	# Poll the world state for valid actions.
	var actions_with_worldly: Array[Action] = []
	actions_with_worldly.append_array(self_actions)
	actions_with_worldly.append_array(worldly_actions)

	# Deterimine the rewards from each possible goal.
	var highest_reward_goal: Goal = goals[0]
	for goal in goals:
		rewards.append(goal.compute_reward(self))
	# Continually check for the most rewarding goal.
	while rewards.max() > -1:
		var max_reward: float = rewards.max()
		var idx: int = rewards.find(max_reward)
		var test_plan: Plan = Plan.new()
		test_plan.initialize(self, goals[idx], actions_with_worldly, config.max_recursion)

		if test_plan.get_plan().size() > 0: # This means a plan was created.
			return_dict["goal"] = goals[idx]
			return_dict["plan"] = test_plan
			# For multithreading, we need to sync to main thread before exiting.
			if config.use_multithreading:
				call_deferred("_sync_multithreaded_plan")
			return return_dict
		rewards[idx] = -1
		break

	# For multithreading, we need to sync to main thread before exiting.
	if config.use_multithreading:
		call_deferred("_sync_multithreaded_plan")
	return { "goal": null, "plan": null }


## Waits for thread to finish then assigns return values.
func _sync_multithreaded_plan() -> void:
	var goal_and_plan: Dictionary = thread.wait_to_finish()
	_current_goal = goal_and_plan["goal"]
	_current_plan = goal_and_plan["plan"]
	if _current_plan != null:
		_reset_runtime_status(_current_plan.get_plan())
		_update_debugger_info()


## Executes the currently selected plan based on the current step.
func _execute_plan(delta: float) -> void:
	if _current_plan == null: # No plan formed yet.
		return
	var action_chain: Array[Action] = _current_plan.get_plan()
	# Pre actions.
	if _current_plan_step == -1:
		for action: Action in action_chain:
			var action_status: Action.Status = action.pre_perform_action(self)
			_runtime_status[action.uid]["pre_status"] = action_status
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
		_runtime_status[current_action.uid]["runtime_status"] = action_status
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
			var action_status: Action.Status = action.post_perform_action(self)
			_runtime_status[action.uid]["post_status"] = action_status
		_current_plan_step += 1
	# Update debugger info.
	_update_debugger_info()


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
	var gdpai_objects: Array[GdPAIObjectData] = ws_checkpoint.get_property(
		GdPAIBlackboard.GDPAI_OBJECTS,
	)
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


func _reset_runtime_status(plan_actions: Array[Action]) -> void:
	_runtime_status.clear()
	for action in plan_actions:
		_runtime_status[action.uid] = {
			"uid": action.uid,
			"pre_status": "...",
			"runtime_status": "...",
			"post_status": "...",
		}


func _inject_runtime_status(node: Dictionary) -> Dictionary:
	var uid: String = node.get("id", "")
	if uid not in _runtime_status:
		return node
	node["pre_status"] = _runtime_status[uid]["pre_status"]
	node["runtime_status"] = _runtime_status[uid]["runtime_status"]
	node["post_status"] = _runtime_status[uid]["post_status"]

	if node.has("children"):
		var updated_children: Array[Dictionary] = []
		for child in node.get("children", []):
			updated_children.append(_inject_runtime_status(child))
		node["children"] = updated_children
	return node


func _update_debugger_info() -> void:
	var agent_info: Dictionary = { }
	# Setup the plan tree if we have a valid plan.
	if _current_plan != null and is_instance_valid(_current_plan):
		agent_info["plan_tree"] = _inject_runtime_status(_current_plan.get_plan_tree_debug_data())
	# Setup the current goal if we have one.
	if _current_goal != null and is_instance_valid(_current_goal):
		agent_info["current_goal"] = _current_goal.get_title()
		agent_info["current_goal_description"] = _current_goal.get_description()

	EngineDebugger.send_message("gdplanningai:update_agent_info", [get_instance_id(), agent_info])
