# meta-description: GdPAI Goal template.
# meta-default: true
extends Goal


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	return 0


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	return []


# Override
func get_title() -> String:
	return "Goal"


# Override
func get_description() -> String:
	return "Base class for Goal."
