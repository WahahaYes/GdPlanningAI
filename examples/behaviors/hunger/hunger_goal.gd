class_name HungerGoal
extends Goal
## Goal that drives an agent to seek food when hungry.
##[br]
##[br]
## Reward scales from 0 (full) to 100 (empty), so the planner prioritizes eating
## over wandering whenever hunger is meaningfully low.
##[br]
##[br]
## Below [member sated_hunger] the goal is already satisfied, so the planner skips
## it instead of searching for a hunger level no meal can reach.

## Hunger below this counts as sated.
var sated_hunger: float = 15.0
## How far one meal should bring hunger down, when that stays above sated.
var hunger_drop: float = 15.0


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	var hunger_val = agent.blackboard.get_property("hunger")
	if hunger_val == null:
		return 0.0
	return max(0.0, float(hunger_val))


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	var current_hunger: float = agent.blackboard.get_property("hunger")
	var required_hunger: float = max(sated_hunger, current_hunger - hunger_drop)

	# Using a builtin precondition instead of a custom callable allows the planner
	# to correctly identify which actions satisfy this goal during the discovery phase.
	return [Precondition.agent_property_less_than("hunger", required_hunger)]


# Override
func get_title() -> String:
	return "Hunger"


# Override
func get_description() -> String:
	return "Eat food and keep hunger up."
