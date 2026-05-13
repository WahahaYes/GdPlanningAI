class_name AddFuelAction
extends Action

const ADD_FUEL_DURATION: float = 1.0

var campfire_ref: CampfireObject
var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
var fuel_per_wood: float


func _init(
	p_campfire: CampfireObject,
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
	p_fuel_per_wood: float,
) -> void:
	campfire_ref = p_campfire
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs
	fuel_per_wood = p_fuel_per_wood


func get_validity_checks() -> Array[Precondition]:
	return []


func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("held_item", "wood")]


func get_requirements() -> Array[RequirementSpec]:
	if object_location != null:
		return [RequirementSpec.fact("at_target", [object_location])]
	return []


func get_provisions() -> Array[ProvisionSpec]:
	return []


func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> float:
	if not is_instance_valid(campfire_ref):
		return INF
	var campfire: SimObjectProxy = world_state.get_object_for(campfire_ref)
	if campfire == null:
		return INF
	var current_fuel: Variant = campfire.get_property("current_fuel")
	if current_fuel == null or float(current_fuel) >= 100.0:
		return INF
	return ADD_FUEL_DURATION


func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> void:
	var campfire: SimObjectProxy = world_state.get_object_for(campfire_ref)
	if campfire != null:
		var current_fuel: Variant = campfire.get_property("current_fuel")
		if current_fuel != null:
			campfire.set_property("current_fuel", min(100.0, float(current_fuel) + fuel_per_wood))
	agent_blackboard.set_property("held_item", "")


func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	set_state(agent, "add_fuel_elapsed", 0.0)
	return Action.Status.SUCCESS


func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	if not is_instance_valid(campfire_ref):
		return Action.Status.FAILURE

	var elapsed: float = get_state(agent, "add_fuel_elapsed") + delta
	set_state(agent, "add_fuel_elapsed", elapsed)

	if elapsed >= ADD_FUEL_DURATION:
		campfire_ref.current_fuel = min(100.0, campfire_ref.current_fuel + fuel_per_wood)
		agent.blackboard.set_property("held_item", "")
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	erase_state(agent, "add_fuel_elapsed")
	return Action.Status.SUCCESS


func get_title() -> String:
	return "Add Fuel"


func get_description() -> String:
	return "Add wood to the campfire to increase its fuel level."
