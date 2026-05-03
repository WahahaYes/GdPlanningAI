class_name PickupAction
extends SpatialAction
## Navigates an agent to a [HoldableObject] and picks it up.
##[br]
##[br]
## Demonstrates a reusable pickup action for world objects that write an item id
## into the agent's [code]held_item[/code] blackboard property.

## Reference to the holdable item that provided this action.
var holdable_item: HoldableObject


# Override
func _init(
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
	p_holdable_item: HoldableObject,
) -> void:
	super(p_object_location, p_interactable_attribs)
	holdable_item = p_holdable_item


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = super()
	checks.append(Precondition.check_is_object_valid(holdable_item))
	return checks


# Override
func get_action_cost(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> float:
	return super(agent_blackboard, world_state)


# Override
func get_preconditions() -> Array[Precondition]:
	var has_empty_hands = func(
		blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
	) -> bool:
		var held_item = blackboard.get_property("held_item")
		return held_item == null or held_item == ""
	return [Precondition.custom(has_empty_hands)]


# Override
func get_provisions() -> Array[ProvisionSpec]:
	# Provide the held_item binding that EatHeldFoodAction requires
	return [ProvisionSpec.binding("held_item", holdable_item.item_id)]


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	super(agent_blackboard, _world_state)
	agent_blackboard.set_property("held_item", holdable_item.item_id)
	print("[PickupAction] simulate_effect - setting held_item to: ", holdable_item.item_id)


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	return super(agent)


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var parent_status: Action.Status = super(agent, delta)
	if parent_status == Action.Status.FAILURE:
		return Action.Status.FAILURE

	if not get_state(agent, "target_reached"):
		return Action.Status.RUNNING

	agent.blackboard.set_property("held_item", holdable_item.item_id)
	holdable_item.entity.queue_free()
	return Action.Status.SUCCESS


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	return super(agent)


# Override
func get_title() -> String:
	return "Pick Up Item"


# Override
func get_description() -> String:
	return "Navigate to and pick up an item."
