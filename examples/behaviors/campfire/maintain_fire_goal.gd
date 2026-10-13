class_name MaintainFireGoal
extends Goal
## Goal that drives an agent to keep campfires above a target fuel threshold.
##[br]
##[br]
## Demonstrates a goal that inspects shared world-object state instead of only the
## agent blackboard. Uses builtin WorldObjectProxy precondition for planner visibility.
##[br]
##[br]
## With [member campfire_group] empty the agent tends any campfire; set it to a
## group holding one campfire to give the agent a home fire.

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
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
	# Use builtin WorldObjectProxy precondition so the planner can discover
	# actions that modify campfire fuel (e.g., AddFuelAction).
	return [
		Precondition.world_object_property_geq_than(
			tended_group(), "current_fuel", desired_fuel_level
		)
	]


# Override
func get_title() -> String:
	return "Maintain Fire"


# Override
func get_description() -> String:
	return "Keep the campfire supplied with enough fuel for cooking."
