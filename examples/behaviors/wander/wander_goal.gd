class_name WanderGoal
extends Goal
## Keep moving when nothing else needs doing (fixed reward 5).[br]
## Satisfied once a plan contains a wander step: [WanderAction] sets [code]wandered[/code] only in
## the simulation, so the real agent never counts as satisfied and keeps wandering.


# Override
func compute_reward(_agent: GdPAIAgent) -> float:
	return 5.0


# Override
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("wandered", true)]


# Override
func get_title() -> String:
	return "Wander"


# Override
func get_description() -> String:
	return "Move around the environment."
