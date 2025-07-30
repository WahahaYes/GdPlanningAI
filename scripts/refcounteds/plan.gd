## Simulates chains of actions recursively to move an agent towards a goal.
class_name Plan
extends RefCounted

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")
## A reference to the agent.
var _agent: GdPAIAgent
## A reference to the goal.
var _goal: Goal
## The currently available actions provided by the agent.
var _available_actions: Array[Action]
## How deep to traverse when forming a plan.
var _max_recursion: int
## After planning, this stores the best valid plan found.
var _best_plan: Array


## Initializes the plan with the target agent, goal, available actions, and a limit to the amount
## of recursion in actions when formulating the plan.
func initialize(agent: GdPAIAgent, goal: Goal, actions: Array[Action], max_recursion: int = 8):
	_agent = agent
	_goal = goal
	_available_actions = actions
	_max_recursion = max_recursion

	_best_plan = await _compute_plan()


## Returns the list of actions needed to arrive at the goal.
func get_plan() -> Array:
	return _best_plan


## Computes the plan recursively by simulating outside of the scene tree.
func _compute_plan() -> Array:
	var blackboard: GdPAIBlackboard = _agent.blackboard.copy_for_simulation()
	blackboard.is_a_copy = true
	var world_state: GdPAIBlackboard = _agent.world_node.world_state.copy_for_simulation()
	world_state.is_a_copy = true

	var root_node: Dictionary = {
		"action": Action.new(),
		"cost": 0,
		"desired_state": _goal.get_desired_state(_agent),  # An array of preconditions.
		"children": [],
	}
	# Attempt to create the full tree of possible plans.
	var success: bool = await _build_plan(root_node, [], blackboard, world_state, 0)
	# Parse out the actions from the most cost effective plan.
	if success:
		var plans: Array = _transform_tree_into_array(root_node)
		var best_plan: Variant = null

		var i = 0
		for p in plans:
			if best_plan == null or p.cost < best_plan.cost:
				best_plan = p
			i += 1
		return best_plan.actions
	else:
		return []


## Builds a plan recursively to find cases where the goal is realized.  If there is no valid
## plan within the recursion limit, returns false.
func _build_plan(
	node: Dictionary,
	prior_actions: Array[Action],
	blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
	recursion_level: int
):
	var has_solution: bool = false
	# Early terminate if recursion went too deep.
	if recursion_level > _max_recursion:
		return false
	# Check if the goal has been realized at this point in the simulation.
	await _evaluate_goals(node["desired_state"], blackboard, world_state)
	if _is_goal_satisfied(node["desired_state"]):
		return true
	# Otherwise, continue to look for viable actions.
	for action in _available_actions:
		# Track if this action will make progress towards our target world state.
		var should_use_action: bool = false
		# Clone the agent and world states for simulation.
		var sim_blackboard: GdPAIBlackboard = blackboard.copy_for_simulation()
		sim_blackboard.is_a_copy = true
		var sim_world_state: GdPAIBlackboard = world_state.copy_for_simulation()
		sim_world_state.is_a_copy = true
		var sim_cost: float = await action.get_action_cost(sim_blackboard, sim_world_state)
		if sim_cost == INF:
			continue
		action.simulate_effect(sim_blackboard, sim_world_state)
		# Backprop with future actions.
		for prior_act: Action in prior_actions:
			prior_act.reverse_simulate_effect(sim_blackboard, sim_world_state)

		# Copy a list of requirements to see if any are satisfied
		var sim_desired_state: Array[Precondition] = []
		for condition: Precondition in node["desired_state"]:
			sim_desired_state.append(condition.copy_for_simulation())
		should_use_action = await _evaluate_goals(
			sim_desired_state, sim_blackboard, sim_world_state
		)

		# Add this action's preconditions to the simulated world state and continue recursing.
		if should_use_action:
			var action_preconditions: Array[Precondition] = await action.get_preconditions()
			# Now check if current state (before the action) satisfies some of the actions'
			# preconditions that may invert because of the action.
			await _evaluate_goals(action_preconditions, blackboard, world_state)
			sim_desired_state.append_array(action_preconditions)
			# Create the next recursive node.
			var next_node = {
				"action": action,
				"cost": sim_cost,
				"desired_state": sim_desired_state,
				"children": []
			}
			# Check if we arrive at our goal eventually by including this action.
			var prior_actions_appended: Array[Action] = prior_actions.duplicate()
			prior_actions_appended.append(action)

			if await _build_plan(
				next_node,
				prior_actions_appended,
				sim_blackboard,
				sim_world_state,
				recursion_level + 1
			):
				# Add this node as a potential solution.
				node.children.append(next_node)
				has_solution = true

	return has_solution


## Traverses the tree of action plans and builds up an array of all the potential plans and their
## scores.
func _transform_tree_into_array(node: Dictionary) -> Array:
	var plans = []

	if node.children.size() == 0:
		plans.append({"actions": [node.action], "cost": node.cost})
		return plans

	for child in node.children:
		for child_plan in _transform_tree_into_array(child):
			child_plan.actions.append(node.action)
			child_plan.cost += node.cost
			plans.append(child_plan)
	return plans


## Run the evaluation on any preconditions not previously addressed.  Returns true if any
## conditions not previously met are satisfied.
func _evaluate_goals(
	preconditions: Array[Precondition], blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard
) -> bool:
	var is_closer_to_goal: bool = false
	for condition: Precondition in preconditions:
		if condition.is_satisfied:  # We don't want a condition previously satisfied to revert.
			continue
		var result: bool = await condition.evaluate(blackboard, world_state)
		if result:
			is_closer_to_goal = true
	return is_closer_to_goal


## Checks over the list of preconditions to verify if all conditions have been previously
## satisfied.  Doesn't attempt to verify any of the conditions.
func _is_goal_satisfied(preconditions: Array[Precondition]):
	# Early terminate if any of the target states are not met.
	for condition: Precondition in preconditions:
		if not condition.is_satisfied:
			return false
	return true
