class_name WanderGoal
extends Goal
## Keep moving when nothing else needs doing (fixed reward 5).[br]
## Satisfied once the simulated agent is far enough from where it started. "Far from the origin"
## is not a named property, so this uses a custom precondition.


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

	# Well under one wander step, so a single step satisfies the goal.
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
