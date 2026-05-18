class_name PickupAction
extends Action
## An action that picks up a holdable item.
## This action requires the agent to be at the target location (via at_target fact)
## and does NOT handle navigation itself - that's handled by GoToAction.

## Reference to the holdable item that provided this action.
var holdable_item: HoldableObject
## The location data for this item - used to specify the at_target requirement.
var object_location: GdPAILocationData
## Reference to the GdPAI interactable attributes.
var interactable_attribs: GdPAIInteractable


# Override
func _init(
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
	p_holdable_item: HoldableObject,
) -> void:
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs
	holdable_item = p_holdable_item


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	# Interaction cost is minimal - navigation is handled by GoToAction
	return 1.0


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.check_is_object_valid(holdable_item))
	checks.append(Precondition.check_is_object_valid(object_location))
	checks.append(Precondition.check_is_object_valid(interactable_attribs))
	return checks


# Override
func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("held_item", "")]


# Override
func get_requirements() -> Array[RequirementSpec]:
	# Require that the agent is at this specific object's location
	if object_location != null:
		return [RequirementSpec.fact("at_target", [object_location])]
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	# Provide the held_item binding that EatHeldFoodAction requires.
	# Also provide an 'is_food' fact if this object belongs to the Food group.
	var provs: Array[ProvisionSpec] = [ProvisionSpec.binding("held_item", holdable_item.item_id)]
	if is_instance_valid(holdable_item) and holdable_item.is_in_group("Food"):
		provs.append(ProvisionSpec.fact("is_food", []))
	return provs


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	# Ensure the holdable item is still valid
	if not is_instance_valid(holdable_item) or not is_instance_valid(holdable_item.entity):
		return

	agent_blackboard.set_property("held_item", holdable_item.item_id)


# Override
func pre_perform_action(_agent: GdPAIAgent) -> Action.Status:
	# Check if the holdable item is still valid
	if not is_instance_valid(holdable_item) or not is_instance_valid(holdable_item.entity):
		return Action.Status.FAILURE
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
	# Check if the holdable item is still valid
	if not is_instance_valid(holdable_item) or not is_instance_valid(holdable_item.entity):
		return Action.Status.FAILURE

	# Agent should already be at the location (GoToAction handled navigation)
	# Just pick up the item
	agent.blackboard.set_property("held_item", holdable_item.item_id)
	holdable_item.entity.queue_free()
	return Action.Status.SUCCESS


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Pick Up Item"


# Override
func get_description() -> String:
	return "Pick up an item (requires GoToAction for navigation)."
