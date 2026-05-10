class_name PickUpWoodAction
extends Action

var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable


func _init(
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
) -> void:
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs


func get_validity_checks() -> Array[Precondition]:
	return [
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
	return [ProvisionSpec.binding("held_item", "wood")]


func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0.5


func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("held_item", "wood")


func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
	agent.blackboard.set_property("held_item", "wood")
	return Action.Status.SUCCESS


func get_title() -> String:
	return "Pick Up Wood"


func get_description() -> String:
	return "Pick up wood from a wood pile (requires GoToAction for navigation)."
