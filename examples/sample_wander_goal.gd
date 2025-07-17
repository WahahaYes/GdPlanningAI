class_name SampleWanderGoal
extends Goal


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	# This goal of 10 is scaled relative to hunger's dynamic goal between 1 - 100.  Essentially,
	# when the agent hunger is 90+ this goal takes priority.  This goal also kicks in if hunger
	# can't be increased.
	return 10


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	var agent_location_data: GdPAILocationData = agent.blackboard.get_first_object_in_group(
		"GdPAILocationData"
	)
	var agent_position = agent_location_data.position

	var move_condition: Precondition = Precondition.new()
	move_condition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		var sim_location_data: GdPAILocationData = blackboard.get_first_object_in_group(
			"GdPAILocationData"
		)
		var sim_position = sim_location_data.position

		# Because this example works for 2D or 3D, just specify that the moved distance should be
		# meaningful.
		var req_distance: float = 16 if sim_location_data.position is Vector2 else 1
		return (sim_position - agent_position).length() > req_distance

	return [move_condition]
