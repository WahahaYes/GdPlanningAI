class_name PickUpWoodAction
extends Action
## Takes a log from a wood pile. First half of the chain Go To (pile), Pick Up Wood, Go To
## (fire), Add Fuel.[br]
## Its [code]held_item == "wood"[/code] binding satisfies [AddFuelAction]. It provides no
## [code]is_food[/code], so eating can never follow it.

## The pile's location, named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable


func _init(
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
) -> void:
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs


# Override
func get_validity_checks() -> Array[Precondition]:
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
## Native form of [method simulate_effect]: the agent ends up holding wood.
func get_effect_spec() -> Array:
	return [
		{"op": "set", "target": "agent", "prop": "held_item", "value": "wood"},
	]


# Override
func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
	agent.blackboard.set_property("held_item", "wood")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Pick Up Wood"


# Override
func get_description() -> String:
	return "Pick up wood from a wood pile (requires GoToAction for navigation)."
