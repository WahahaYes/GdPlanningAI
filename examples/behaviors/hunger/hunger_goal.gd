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
	print("[HungerGoal] get_desired_state - current_hunger: ", current_hunger)

	# Require hunger to be reduced by at least 15 to satisfy the goal
	# This ensures the planner must chain pickup + eat, since eat alone
	# only provides a 5.0 placeholder during planning
	var required_hunger: float = max(0.0, current_hunger - 15.0)

	var check_hunger_less_than = func(
		blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
	) -> bool:
		var hunger: Variant = blackboard.get_property("hunger")
		var result = hunger < required_hunger
		print("[HungerGoal] Custom precondition check")
		print("  hunger: ", hunger, ", threshold: ", required_hunger, ", result: ", result)
		return result

	return [Precondition.custom(check_hunger_less_than)]


# Override


func get_title() -> String:
	return "Hunger"


# Override


func get_description() -> String:
	return "Eat food and keep hunger up."
