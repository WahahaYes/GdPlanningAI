class_name HungerGoal
extends Goal
## Goal that drives an agent to seek food when hungry.
##[br]
##[br]
## Hunger is a 0 to 100 value that rises over time: 0 is full, 100 is starving. The
## reward is the hunger value itself, so the goal's priority grows as the agent
## starves and eventually outranks lower fixed-reward goals such as [WanderGoal] (5) and
## [MaintainFireGoal] (40).[br]
##[br]
## The desired state asks for one meal's worth of improvement, not for hunger 0. A meal
## restores 20 to 50 points, so asking for a full stomach would often be unreachable in
## a single plan. Below [member sated_hunger] the goal is already satisfied, so the
## planner skips it instead of searching for a hunger level no meal can reach.[br]
##[br]
## Read next: [EatHeldFoodAction] for how the goal is met, [HungerPropertyUpdater] for
## where [code]hunger[/code] comes from.

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
	# A Callable is opaque to the planner, so it could only test a finished state; a
	# builtin names the property, which lets the planner search for actions that change it.
	return [Precondition.agent_property_less_than("hunger", required_hunger)]


# Override
func get_title() -> String:
	return "Hunger"


# Override
func get_description() -> String:
	return "Eat food and keep hunger up."
