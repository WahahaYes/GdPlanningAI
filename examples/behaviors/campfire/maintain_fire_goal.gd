class_name MaintainFireGoal
extends Goal
## Goal that drives an agent to keep campfires above a target fuel threshold.
##[br]
##[br]
## Demonstrates a goal that inspects shared world-object state instead of only the
## agent blackboard.

## Fixed reward used when comparing fire maintenance against other goals.
var reward_value: float = 40.0
## Minimum fuel level any campfire must reach for this goal to count as satisfied.
var desired_fuel_level: float = 60.0


func _init(
	p_reward_value: float = 40.0,
	p_desired_fuel_level: float = 60.0,
) -> void:
	reward_value = p_reward_value
	desired_fuel_level = p_desired_fuel_level


# Override
func compute_reward(agent: GdPAIAgent) -> float:
	if agent.world_node == null:
		return reward_value
	var world_state: GdPAIBlackboard = agent.world_node.get_world_state()
	if world_state == null:
		return reward_value
	var campfires: Array[SimObjectProxy] = world_state.get_proxies_in_group("CampfireObject")
	if campfires.is_empty():
		return reward_value
	for campfire in campfires:
		var fuel: Variant = campfire.get_property("current_fuel")
		if fuel != null and float(fuel) < desired_fuel_level:
			return reward_value
	return 0.0


# Override
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
	# Custom precondition: campfire fuel is stored in per-object SimObjectProxy data,
	# not in top-level world blackboard properties, so a builtin check cannot see it.
	var target: float = desired_fuel_level
	var check: Callable = func(_agent_bb: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
		var campfires = world.get_proxies_in_group("CampfireObject")
		# print("[DEBUG] MaintainFireGoal check running. Found ", campfires.size(), " campfires.")
		for campfire in campfires:
			var fuel: Variant = campfire.get_property("current_fuel")
			# print("[DEBUG] Campfire fuel: ", fuel)
			if fuel != null and float(fuel) >= target:
				return true
		return false
	return [Precondition.custom(check)]


# Override
func get_title() -> String:
	return "Maintain Fire"


# Override
func get_description() -> String:
	return "Keep the campfire supplied with enough fuel for cooking."
