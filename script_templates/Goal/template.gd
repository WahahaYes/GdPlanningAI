# meta-description: GdPAI Goal template.
# meta-default: true
extends Goal


## Called every planning cycle to re-prioritize this goal.
## Return value can change frame-to-frame to reflect dynamic conditions (e.g. rising hunger).
# Override
func compute_reward(_agent: GdPAIAgent) -> float:
	return 0


# Override
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
	return []


# Override
func get_title() -> String:
	return "Goal"


# Override
func get_description() -> String:
	return "Base class for Goal."
