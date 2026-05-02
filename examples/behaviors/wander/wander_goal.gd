class_name WanderGoal
extends Goal
## Goal that drives an agent to move around the environment.
##[br]
##[br]
## Fixed reward of 10, making it lower priority than a full-scale hunger crisis but
## high enough to keep the agent moving when nothing else needs doing.


# Override
func compute_reward(_agent: GdPAIAgent) -> float:
	return 10.0


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

	var move_condition: Precondition = Precondition.custom(
		func(
			blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard,
		) -> bool:
			var sim_location: SimObjectProxy = blackboard.get_proxy_in_group("GdPAILocationData")
			if sim_location == null:
				return false
			var sim_position = sim_location.get_property("position")
			var req_distance: float = 16.0 if sim_position is Vector2 else 1.0
			return (sim_position - agent_position).length() > req_distance
	)

	return [move_condition]


# Override


func get_title() -> String:
	return "Wander"


# Override


func get_description() -> String:
	return "Move around the environment."
