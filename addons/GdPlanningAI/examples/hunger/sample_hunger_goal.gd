class_name SampleHungerGoal
extends Goal
## Simple GdPAI goal to eat food and keep hunger up.


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	return 100.0 - agent.blackboard.get_property("hunger", 100.0)


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	var current_hunger: float = agent.blackboard.get_property("hunger")
	return [Precondition.agent_property_greater_than("hunger", current_hunger)]


# Override
func get_title() -> String:
	return "Hunger"


# Override
func get_description() -> String:
	return "Eat food and keep hunger up."
