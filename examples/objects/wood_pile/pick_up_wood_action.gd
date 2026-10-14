class_name PickUpWoodAction
extends Action
## Takes a log from a wood pile into the held-item slot.
##[br]
##[br]
## The first half of the maintain-fire chain: Go To (wood pile), Pick Up Wood, Go To
## (campfire), Add Fuel. Its [b]requirement[/b] [code]at_target[/code] for the pile is
## what makes the planner insert a Go To step, and its [b]provision[/b] of a
## [code]held_item == "wood"[/code] binding is what satisfies [AddFuelAction]'s
## requirement. The wildcard [code]at_target[/code] fact from [GoToAction] satisfies
## each interaction separately, so the same Go To action appears twice in the plan with
## different targets (the pile, then the fire).[br]
##[br]
## Unlike [PickupAction] it provides no [code]is_food[/code] fact, so
## [EatHeldFoodAction] can never be chained after it: that fact is the line between
## food and wood. It also frees nothing, because the pile is not consumed and stays
## for the next trip.[br]
##[br]
## The cost is a small constant (0.5), so travel distance, which Go To supplies,
## dominates the choice between piles.

## The pile's location; named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
## The pile's interactable component, kept for parity with the other object actions.
var interactable_attribs: GdPAIInteractable


func _init(
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
) -> void:
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs


# Override
func get_validity_checks() -> Array[Precondition]:
	# The pile is never freed or depleted, so there is nothing to check.
	return []


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
	return [ProvisionSpec.binding("held_item", "wood")]


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0.5


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("held_item", "wood")


# Override
func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
	# Instant and infallible: the pile is never consumed, so nothing can invalidate the pick-up.
	agent.blackboard.set_property("held_item", "wood")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Pick Up Wood"


# Override
func get_description() -> String:
	return "Pick up wood from a wood pile (requires GoToAction for navigation)."
