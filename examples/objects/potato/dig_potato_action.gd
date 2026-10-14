class_name DigPotatoAction
extends Action
## Digs up a raw potato into the agent's single held-item slot, freeing the potato. Start of
## the chain Go To (potato), Dig, Go To (fire), Cook, Eat.[br]
## Potatoes are not reserved: an agent arriving second fails and replans.

## The potato's location, named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
## The potato being dug; freed on success.
var potato_ref: PotatoObject


func _init(
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
	p_potato_ref: PotatoObject,
) -> void:
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs
	potato_ref = p_potato_ref


# Override
func get_validity_checks() -> Array[Precondition]:
	return [
		Precondition.check_is_object_valid(potato_ref),
		Precondition.check_is_object_valid(potato_ref.entity),
		Precondition.check_is_object_valid(object_location),
		Precondition.check_is_object_valid(interactable_attribs),
	]


# Override
func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("held_item", "")]


# Override
func get_requirements() -> Array[RequirementSpec]:
	if object_location != null:
		return [RequirementSpec.fact("at_target", [object_location])]
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	return [ProvisionSpec.binding("held_item", "potato"), ProvisionSpec.fact("is_food", [])]


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0.7


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("held_item", "potato")


# Override
func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
	# Another agent may have dug this potato since planning.
	if not is_instance_valid(potato_ref) or not is_instance_valid(potato_ref.entity):
		return Action.Status.FAILURE
	agent.blackboard.set_property("held_item", "potato")
	potato_ref.entity.queue_free()
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Dig Potato"


# Override
func get_description() -> String:
	return "Dig up a raw potato from the ground (requires GoToAction for navigation)."
