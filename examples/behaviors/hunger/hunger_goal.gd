class_name HungerGoal
extends Goal
## Goal that drives an agent to seek food when hungry.
##[br]
##[br]
## Reward scales from 0 (full) to 100 (empty), so the planner prioritizes eating
## over wandering whenever hunger is meaningfully low.


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	var hunger_val = agent.blackboard.get_property("hunger")
	if hunger_val == null:
		return 0.0
	return max(0.0, float(hunger_val))


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	var current_hunger: float = agent.blackboard.get_property("hunger")
	return [Precondition.agent_property_less_than("hunger", current_hunger)]


# Override
func get_title() -> String:
	return "Hunger"


# Override
func get_description() -> String:
	return "Eat food and keep hunger up."
