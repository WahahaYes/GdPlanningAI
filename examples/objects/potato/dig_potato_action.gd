class_name DigPotatoAction
extends Action

var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
var potato_ref: PotatoObject


func _init(
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
	p_potato_ref: PotatoObject,
) -> void:
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs
	potato_ref = p_potato_ref


func get_validity_checks() -> Array[Precondition]:
	return [
		Precondition.check_is_object_valid(potato_ref),
		Precondition.check_is_object_valid(potato_ref.entity),
		Precondition.check_is_object_valid(object_location),
		Precondition.check_is_object_valid(interactable_attribs),
	]


func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("held_item", "")]


func get_requirements() -> Array[RequirementSpec]:
	if object_location != null:
		return [RequirementSpec.fact("at_target", [object_location])]
	return []


func get_provisions() -> Array[ProvisionSpec]:
	return [ProvisionSpec.binding("held_item", "potato")]


func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0.7


func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("held_item", "potato")


func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
	if not is_instance_valid(potato_ref) or not is_instance_valid(potato_ref.entity):
		return Action.Status.FAILURE
	agent.blackboard.set_property("held_item", "potato")
	potato_ref.entity.queue_free()
	return Action.Status.SUCCESS


func get_title() -> String:
	return "Dig Potato"


func get_description() -> String:
	return "Dig up a raw potato from the ground (requires GoToAction for navigation)."
