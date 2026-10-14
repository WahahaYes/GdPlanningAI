class_name HungerGoal
extends Goal
## Seek food when hungry. Reward is the hunger value (0 to 100), so it eventually outranks
## fixed-reward goals.[br]
## Each plan asks for one meal's improvement, not hunger 0, which a single meal often cannot
## reach. Below [member sated_hunger] the goal is already satisfied.

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

	# Builtin, so the planner can find actions that lower hunger.
	return [Precondition.agent_property_less_than("hunger", required_hunger)]


# Override
func get_title() -> String:
	return "Hunger"


# Override
func get_description() -> String:
	return "Eat food and keep hunger up."
