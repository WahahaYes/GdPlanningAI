class_name PickupAction
extends Action
## Picks up a holdable item, freeing it. Navigation is GoTo's job, via [code]at_target[/code].[br]
## Provides [code]held_item = <item_id>[/code], plus [code]is_food[/code] for items in the
## [code]Food[/code] group. Items are not reserved: an agent arriving second fails and replans.

## The item that offers this action.
var holdable_item: HoldableObject
## The item's location, named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
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
	# GoTo's distance-based cost decides between items.
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
	if object_location != null:
		return [RequirementSpec.fact("at_target", [object_location])]
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	if not is_instance_valid(holdable_item):
		return []
	var provs: Array[ProvisionSpec] = [ProvisionSpec.binding("held_item", holdable_item.item_id)]
	if holdable_item.is_in_group("Food"):
		provs.append(ProvisionSpec.fact("is_food", []))
	return provs


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	if not is_instance_valid(holdable_item) or not is_instance_valid(holdable_item.entity):
		return

	agent_blackboard.set_property("held_item", holdable_item.item_id)


# Override
func pre_perform_action(_agent: GdPAIAgent) -> Action.Status:
	if not is_instance_valid(holdable_item) or not is_instance_valid(holdable_item.entity):
		return Action.Status.FAILURE
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
	if not is_instance_valid(holdable_item) or not is_instance_valid(holdable_item.entity):
		return Action.Status.FAILURE

	agent.blackboard.set_property("held_item", holdable_item.item_id)
	holdable_item.entity.queue_free()
	return Action.Status.SUCCESS


# Override
func post_perform_action(_agent: GdPAIAgent) -> Action.Status:
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Pick Up Item"


# Override
func get_description() -> String:
	return "Pick up an item (requires GoToAction for navigation)."
