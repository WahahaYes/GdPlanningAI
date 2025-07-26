# meta-description: GdPAI Goal template.
# meta-default: true

extends Goal


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	return 0


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	return []
