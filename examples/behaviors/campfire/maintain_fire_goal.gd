class_name MaintainFireGoal
extends Goal
## Keeps campfires above [member desired_fuel_level], reading fuel from world-object state.[br]
## Each plan asks for a small rise ([constant FUEL_STEP]), which one wood delivery always
## satisfies; repeating climbs back to the threshold. Asking for the full level at once could
## exceed the search depth. The reward is flat, and 0 once every tended fire is full.

## Rise in fuel one plan asks for.
const FUEL_STEP: float = 1.0

## Reward while any tended fire is below the threshold.
var reward_value: float = 40.0
## Fuel every tended fire should reach.
var desired_fuel_level: float = 60.0
## Group of the tended campfires. Empty means any campfire.
var campfire_group: String = ""


func _init(
	p_reward_value: float = 40.0,
	p_desired_fuel_level: float = 60.0,
	p_campfire_group: String = "",
) -> void:
	reward_value = p_reward_value
	desired_fuel_level = p_desired_fuel_level
	campfire_group = p_campfire_group


## Returns [member campfire_group], or every campfire when it is empty.
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
	# Builtin, so the planner can find actions that raise current_fuel.
	return [
		Precondition.world_object_property_geq_than(
			tended_group(), "current_fuel", target_fuel(_highest_fuel(agent))
		)
	]


## Returns the fuel one plan should reach: a step above [param highest_fuel] (negative when
## unknown), capped at [member desired_fuel_level].
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
