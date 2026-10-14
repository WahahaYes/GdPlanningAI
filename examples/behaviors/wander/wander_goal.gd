class_name WanderGoal
extends Goal
## Goal that drives an agent to move around the environment.
##[br]
##[br]
## Fixed reward of 5, lower than any hunger above 5 but above zero so the agent keeps
## moving when nothing else needs doing.
##[br]
##[br]
## The goal is satisfied when the agent's simulated position ends up far enough from
## where wandering started, which only [WanderAction] can cause in simulation. The check
## is a custom [Callable] precondition because "far from the origin" is not a property
## the planner could target by name. Unlike [HungerGoal] it needs no builtin
## precondition: the single [WanderAction] is always a candidate.


# Override
func compute_reward(_agent: GdPAIAgent) -> float:
	return 5.0


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	var agent_location_data: GdPAILocationData = (
		agent
		. blackboard
		. get_node_in_group(
			"GdPAILocationData",
		)
	)
	var agent_position = agent_location_data.position

	# Distance threshold is 16 in 2D (pixels) and 1 in 3D (meters), well under the
	# action's step, so one wander step always satisfies the goal.
	var far_enough: Callable = func(
		blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
	) -> bool:
		var sim_location: SimObjectProxy = blackboard.get_proxy_in_group("GdPAILocationData")
		if sim_location == null:
			return false
		var sim_position = sim_location.get_property("position")
		var origin = sim_location.get_property("wander_origin")
		if origin == null:
			origin = agent_position
		var req_distance: float = 16.0 if sim_position is Vector2 else 1.0
		return (sim_position - origin).length() > req_distance
	return [Precondition.custom(far_enough)]


# Override
func get_title() -> String:
	return "Wander"


# Override
func get_description() -> String:
	return "Move around the environment."
