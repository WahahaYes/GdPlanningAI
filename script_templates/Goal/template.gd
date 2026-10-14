# meta-description: GdPAI Goal template.
# meta-default: true
extends Goal


# Override
func compute_reward(_agent: GdPAIAgent) -> float:
	# Read once per plan request; the highest-reward goal with a plan wins.
	# May change over time to reflect dynamic needs (e.g. rising hunger).
	return 0


# Override
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
	# Conditions that must all hold for the goal to be achieved. Prefer built-in
	# preconditions: the planner can search for actions that make them true.
	return []


# Override
func get_title() -> String:
	return "Goal"


# Override
func get_description() -> String:
	return ""
