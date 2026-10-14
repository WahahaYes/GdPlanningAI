class_name DigPotatoAction
extends Action
## Digs a raw potato out of the ground into the held-item slot.
##[br]
##[br]
## The start of the cook-and-eat chain: Go To (potato), Dig Potato, Go To (campfire),
## Cook Potato, Eat Held Food. It behaves like [PickupAction] with a fixed item id: the
## potato node is freed (so a later agent cannot dig the same one) and the agent's
## [code]held_item[/code] becomes [code]"potato"[/code].[br]
##[br]
## It requires [code]held_item == ""[/code] as a [b]precondition[/b] because the agent has
## a single held-item slot: a full slot blocks digging until [DropItemAction] or another
## action empties it. The cost is a small constant (0.7), so travel distance supplied by
## Go To dominates the choice between potatoes.[br]
##[br]
## No claim or reservation is made on the potato. Two agents can head for the same
## one; whoever arrives second finds it freed, [method perform_action] returns
## failure, and that agent replans. The examples accept this to stay simple.

## The potato's location; named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
## The potato's interactable component, kept for parity with the other object actions.
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
	# Validity checks run when actions are collected, so a potato that was already freed
	# (dug by another agent, or removed) never reaches the planner.
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
	# Digging is instant, so there is no pre/post_perform or timer state. The validity
	# check is repeated here because another agent may have dug this potato after planning.
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
