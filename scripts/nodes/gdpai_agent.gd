## Advanced Action Planning Agent.  The agent has their own record of the world state and a
## personal blackboard of attributes.  Agents form plans given their available goals and actions.
class_name GdPAIAgent
extends Node

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")

## Set true if this agent should form its plans in its own thread.
@export var use_multithreading: bool
var _thread: Thread
var _mutex: Mutex

## The top-level node of the agent.
@export var entity: Node
## Reference the plan for this agent's own blackboard.
@export var blackboard_plan: GdPAIBlackboardPlan
## Reference to this agent's own blackboard.
var blackboard: GdPAIBlackboard
## Reference to the world node.
var world_node: GdPAIWorldNode
## List of this agent's goals.
var goals: Array[Goal]
## List of this agent's available actions.
var self_actions: Array[Action]
## The currently selected goal.
var _current_goal: Goal
## The current plan being followed.
var _current_plan: Plan
var _current_plan_step: int = -1


func _ready() -> void:
	blackboard = blackboard_plan.generate_blackboard()

	# Initial blackboard setup common for all agents.
	blackboard.set_property("entity", entity)
	# Collect any GdPAI nodes under this agent's entity.
	var GdPAI_objects: Array[GdPAIObjectData] = []
	for obj: GdPAIObjectData in GdPAIUTILS.get_children_of_type(entity, GdPAIObjectData):
		GdPAI_objects.append(obj)
	blackboard.set_property(GdPAIBlackboard.GdPAI_OBJECTS, GdPAI_objects)
	# Try to find a world node.
	world_node = GdPAIUTILS.get_child_of_type(get_tree().root, GdPAIWorldNode)

	if use_multithreading:
		_thread = Thread.new()
		_mutex = Mutex.new()


func _process(delta: float):
	# Until some goals and actions have been provided, this agent is effectively turned off.
	if goals.size() == 0:
		return
	# Check if a new plan is needed.
	if _current_plan == null or _current_plan_step > _current_plan.get_plan().size():
		var worldly_actions: Array[Action] = await _compute_worldly_actions()
		if use_multithreading:
			if not _thread.is_started():
				# Spin up a new thread to handle planning.
				_thread.start(_select_highest_reward_goal.bind(worldly_actions))
				print("started thread %s" % _thread.get_id())
			else:
				# A thread is currently planning, so we should wait.
				pass
		else:
			var goal_and_plan: Dictionary = _select_highest_reward_goal(worldly_actions)
			_current_goal = goal_and_plan["goal"]
			_current_plan = goal_and_plan["plan"]
			_current_plan_step = -1
	_execute_plan(delta)


func _collect_actions() -> Array[Action]:
	# Collect possible actions.
	var actions_with_worldly: Array[Action] = []
	actions_with_worldly.append_array(self_actions)
	actions_with_worldly.append_array(await _compute_worldly_actions())
	return actions_with_worldly


func _select_highest_reward_goal(worldly_actions: Array[Action]) -> Dictionary:
	if not GdPAIUTILS.am_I_on_main_thread():
		# Removing safety checks usually isn't a good idea.  In our processing we will read from
		# the scene tree but should never write.  With some checks to ensure that data exists, and
		# knowing that this thread's processing is constrained to planning, this is okay.
		_thread.set_thread_safety_checks_enabled(false)

	_mutex.lock()

	var return_dict: Dictionary = {}
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
		var test_plan = Plan.new()
		test_plan.initialize(self, goals[idx], actions_with_worldly)
		if test_plan.get_plan().size() > 0:  # This means a plan was created.
			return_dict["goal"] = goals[idx]
			return_dict["plan"] = test_plan
			# For multithreading, we need to sync to main thread before exiting.
			if use_multithreading:
				call_deferred("_sync_multithreaded_plan")
				_mutex.unlock()
			return return_dict
		else:
			rewards[idx] = -1

	# For multithreading, we need to sync to main thread before exiting.
	if use_multithreading:
		call_deferred("_sync_multithreaded_plan")
		_mutex.unlock()
	return {"goal": null, "plan": null}


func _sync_multithreaded_plan():
	print("collected thread %s" % _thread.get_id())
	var goal_and_plan = _thread.wait_to_finish()

	_current_goal = goal_and_plan["goal"]
	_current_plan = goal_and_plan["plan"]
	_current_plan_step = -1


func _execute_plan(delta: float):
	if _current_plan == null:  # No plan formed yet.
		return

	var action_chain: Array = _current_plan.get_plan()
	# Pre actions.
	if _current_plan_step == -1:
		for action: Action in action_chain:
			var action_status = action.pre_perform_action(self)
			if action_status == Action.Status.FAILURE:
				# Abort the plan.
				_current_plan_step = action_chain.size()
		# Progress to actions if we passed through the preaction stage.
		if _current_plan_step == -1:
			_current_plan_step += 1

	# Actions.
	if _current_plan_step < action_chain.size():
		var current_action = action_chain[_current_plan_step]
		var action_status = current_action.perform_action(self, delta)
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
	if _current_plan_step == action_chain.size():  # We just finished, do post actions.
		for action: Action in action_chain:
			var action_status = action.post_perform_action(self)
		_current_plan_step += 1


## Polls all GdPAI objects to get their relevant actions on request.
func _compute_worldly_actions() -> Array[Action]:
	# Refresh the world state.
	var ws_checkpoint: GdPAIBlackboard = world_node.get_world_state()
	var GdPAI_objects: Array = ws_checkpoint.get_property(GdPAIBlackboard.GdPAI_OBJECTS)
	var actions: Array[Action] = []
	for GdPAI_object: GdPAIObjectData in GdPAI_objects:
		var obj_actions = GdPAI_object.get_provided_actions()
		for obj_act in obj_actions:
			# Every action has a set of validity checks which must pass.
			var validity_checks: Array[Precondition] = obj_act.get_validity_checks()
			var is_satisfied: bool = true
			for check: Precondition in validity_checks:
				var status: bool = await check.evaluate(blackboard, ws_checkpoint)
				if not status:
					is_satisfied = false
					break
			if is_satisfied:
				actions.append(obj_act.copy_for_simulation())
	return actions
