class_name MaintainFireGoal
extends Goal
## Goal that drives an agent to keep campfires above a target fuel threshold.
##[br]
##[br]
## Demonstrates a goal that inspects shared world-object state instead of only the
## agent blackboard. The fuel lives on the campfire, not on the agent, so both the
## reward and the desired state read it from the world state. The desired state uses a
## builtin world-object precondition rather than a [Callable] so the planner can see
## which property it targets and discover [AddFuelAction] as the action that raises it.
##[br]
##[br]
## With [member campfire_group] empty the agent tends any campfire; set it to a
## group holding one campfire to give the agent a home fire.
##[br]
##[br]
## Each plan asks only for the fire to rise (one wood delivery), like [HungerGoal]
## asks for one meal: a full refuel from a low fire needs more actions than an
## agent plans at once. The flat reward keeps the agent delivering until
## [member desired_fuel_level].[br]
##[br]
## Aiming at the full threshold from a nearly dead fire would need several
## pick-up and add-fuel rounds, more than the planner's search depth, so no plan would
## be found and a low fire would stay low. Asking for a small rise always has a
## four-step answer (Go To, Pick Up Wood, Go To, Add Fuel) and repeating it climbs
## back to the threshold, which is why a low fire stays recoverable.[br]
##[br]
## The reward is a flat number, not a function of how low the fire is, so it beats
## [HungerGoal] only while hunger is below the reward and never ranks above a truly
## starving agent. It is 0 once every tended fire is at or above the threshold,
## so the goal drops out of consideration instead of hoarding wood.

## Rise in fuel one plan asks for; any single delivery covers it.
const FUEL_STEP: float = 1.0

## Fixed reward used when comparing fire maintenance against other goals.
var reward_value: float = 40.0
## Minimum fuel level any campfire must reach for this goal to count as satisfied.
var desired_fuel_level: float = 60.0
## Group of the campfires this agent tends. Empty means any campfire.
var campfire_group: String = ""


func _init(
	p_reward_value: float = 40.0,
	p_desired_fuel_level: float = 60.0,
	p_campfire_group: String = "",
) -> void:
	reward_value = p_reward_value
	desired_fuel_level = p_desired_fuel_level
	campfire_group = p_campfire_group


## Returns the group this goal checks: [member campfire_group], or every campfire.
## Scenes with one shared fire leave the group empty; the large 3D scene gives each
## pair of agents a home fire by setting it at spawn time.
func tended_group() -> String:
	return campfire_group if not campfire_group.is_empty() else "CampfireObject"


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	if agent.world_node == null:
		return reward_value
	var world_state: GdPAIBlackboard = agent.world_node.get_world_state()
	if world_state == null:
		return reward_value
	var campfires: Array[SimObjectProxy] = world_state.get_proxies_in_group(tended_group())
	if campfires.is_empty():
		return reward_value
	for campfire in campfires:
		var fuel: Variant = campfire.get_property("current_fuel")
		if fuel != null and float(fuel) < desired_fuel_level:
			return reward_value
	return 0.0


# Override
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	# Use builtin WorldObjectProxy precondition so the planner can discover
	# actions that modify campfire fuel (e.g., AddFuelAction).
	return [
		Precondition.world_object_property_geq_than(
			tended_group(), "current_fuel", target_fuel(_highest_fuel(agent))
		)
	]


## Fuel level one plan should reach, given the tended fires' current best level
## (negative when unknown): a step up from it, capped at [member desired_fuel_level].
func target_fuel(highest_fuel: float) -> float:
	if highest_fuel < 0.0:
		return desired_fuel_level
	return minf(desired_fuel_level, highest_fuel + FUEL_STEP)


## Highest fuel among the tended campfires, or -1 when the world is unavailable.
func _highest_fuel(agent: GdPAIAgent) -> float:
	if agent == null or agent.world_node == null:
		return -1.0
	var world_state: GdPAIBlackboard = agent.world_node.get_world_state()
	if world_state == null:
		return -1.0
	var highest: float = -1.0
	for campfire in world_state.get_proxies_in_group(tended_group()):
		var fuel: Variant = campfire.get_property("current_fuel")
		if fuel != null:
			highest = maxf(highest, float(fuel))
	return highest


# Override
func get_title() -> String:
	return "Maintain Fire"


# Override
func get_description() -> String:
	return "Keep the campfire supplied with enough fuel for cooking."
