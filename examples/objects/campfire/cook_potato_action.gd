class_name CookPotatoAction
extends Action

const COOK_DURATION: float = 2.0

var campfire_ref: CampfireObject
var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
var min_fuel_to_cook: float


func _init(
	p_campfire: CampfireObject,
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
	p_min_fuel_to_cook: float,
) -> void:
	campfire_ref = p_campfire
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs
	min_fuel_to_cook = p_min_fuel_to_cook


func get_validity_checks() -> Array[Precondition]:
	return [
		Precondition.check_is_object_valid(campfire_ref),
		Precondition.check_is_object_valid(object_location),
		Precondition.check_is_object_valid(interactable_attribs),
	]


func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("held_item", "potato")]


func get_requirements() -> Array[RequirementSpec]:
	var reqs: Array[RequirementSpec] = [RequirementSpec.binding_equals("held_item", "potato")]
	if object_location != null:
		reqs.append(RequirementSpec.fact("at_target", [object_location]))
	return reqs


func get_provisions() -> Array[ProvisionSpec]:
	return [ProvisionSpec.binding("held_item", "cooked_potato"), ProvisionSpec.fact("is_food", [])]


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
	if current_fuel == null or float(current_fuel) < min_fuel_to_cook:
		return INF
	return COOK_DURATION


func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("held_item", "cooked_potato")


func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	set_state(agent, "cook_elapsed", 0.0)
	return Action.Status.SUCCESS


func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	if not is_instance_valid(campfire_ref):
		return Action.Status.FAILURE
	if campfire_ref.current_fuel < min_fuel_to_cook:
		return Action.Status.FAILURE

	var elapsed: float = get_state(agent, "cook_elapsed") + delta
	set_state(agent, "cook_elapsed", elapsed)

	if elapsed >= COOK_DURATION:
		agent.blackboard.set_property("held_item", "cooked_potato")
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	erase_state(agent, "cook_elapsed")
	return Action.Status.SUCCESS


func get_title() -> String:
	return "Cook Potato"


func get_description() -> String:
	return "Cook a raw potato at the campfire (requires minimum fuel level)."
