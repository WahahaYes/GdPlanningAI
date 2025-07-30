## GdPlanningAI Agent.  The agent has their own record of the world state and a
## personal blackboard of attributes.  Agents form plans given their available goals and actions.
class_name GdPAIAgent
extends Node

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")
## @experimental: To support multithreading, your simulation must not directly change the scene tree!
## Whether this agent should do planning on a separate thread.
@export var use_multithreading: bool
@export var thread_priority: Thread.Priority = Thread.PRIORITY_LOW
## Agent has ownership over one thread during its lifespan.
var thread: Thread
## The maximum recursion depth for the agent when planning.
@export var max_recursion: int = 4
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
## The current step within a plan.  Also used to control flow for pre/post actions.
var _current_plan_step: int = -1


func _ready() -> void:
	if use_multithreading:
		thread = Thread.new()
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


func _process(delta: float):
	# Until some goals and actions have been provided, this agent is effectively turned off.
	if goals.size() == 0:
		return
	# Check if a new plan is needed.
	if _current_plan == null or _current_plan_step > _current_plan.get_plan().size():
		# Query the world state at the start of planning.
		var worldly_actions: Array[Action] = await _compute_worldly_actions()
		if use_multithreading:
			# Spin up a thread to call the planning logic.  Variable assignment happens at the
			# end of planning.
			if not thread.is_started():
				# Spin up a new thread to handle planning.
				_current_plan = null
				_current_plan_step = -1
				thread.start(_select_highest_reward_goal.bind(worldly_actions), thread_priority)
		else:
			var goal_and_plan: Dictionary = await _select_highest_reward_goal(worldly_actions)
			_current_plan_step = -1
			_current_goal = goal_and_plan["goal"]
			_current_plan = goal_and_plan["plan"]
	_execute_plan(delta)


## Iterates over all goals in order of reward until a valid plan is found.
func _select_highest_reward_goal(worldly_actions: Array[Action]) -> Dictionary:
	if use_multithreading:
		# Removing safety checks usually isn't a good idea.  In our processing we will read from
		# the scene tree or will await information, but we don't write.  With some checks to
		# ensure that data exists, and knowing that this thread's processing is constrained to
		# planning, this is maybe okay to do.
		thread.set_thread_safety_checks_enabled(false)

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
		test_plan.initialize(self, goals[idx], actions_with_worldly, max_recursion)

		if test_plan.get_plan().size() > 0:  # This means a plan was created.
			return_dict["goal"] = goals[idx]
			return_dict["plan"] = test_plan
			# For multithreading, we need to sync to main thread before exiting.
			if use_multithreading:
				call_deferred("_sync_multithreaded_plan")
			return return_dict
		else:
			rewards[idx] = -1
		break

	# For multithreading, we need to sync to main thread before exiting.
	if use_multithreading:
		call_deferred("_sync_multithreaded_plan")
	return {"goal": null, "plan": null}


## Waits for thread to finish then assigns return values.
func _sync_multithreaded_plan():
	var goal_and_plan = thread.wait_to_finish()
	_current_goal = goal_and_plan["goal"]
	_current_plan = goal_and_plan["plan"]


## Executes the currently selected plan based on the current step.
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
	var GdPAI_objects: Array[GdPAIObjectData] = ws_checkpoint.get_property(
		GdPAIBlackboard.GdPAI_OBJECTS
	)
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
				actions.append(obj_act)
	return actions
