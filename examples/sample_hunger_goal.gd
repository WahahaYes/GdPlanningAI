## Simple GdPAI goal to eat food and keep hunger up.
class_name SampleHungerGoal
extends Goal


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	return 100 - agent.blackboard.get_property("hunger")


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	var current_hunger: float = agent.blackboard.get_property("hunger")

	var condition: Precondition = Precondition.new()
	condition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return blackboard.get_property("hunger") > current_hunger

	return [condition]
