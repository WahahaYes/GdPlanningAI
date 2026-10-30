class_name CookPotatoAction
extends Action
## Cooks a held raw potato at a campfire. Middle of the chain Go To (potato), Dig, Go To (fire),
## Cook, Eat.[br]
## Eating only accepts [code]cooked_potato[/code], so cooking is required. Cost is
## [constant INF] below [member min_fuel_to_cook]; this is where the hunger and fire goals meet.

## Seconds spent cooking; also the planning cost.
const COOK_DURATION: float = 2.0

var campfire_ref: CampfireObject
## The campfire's location, named in the [code]at_target[/code] requirement.
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


# Override
func get_validity_checks() -> Array[Precondition]:
	return [
		Precondition.check_is_object_valid(campfire_ref),
		Precondition.check_is_object_valid(object_location),
		Precondition.check_is_object_valid(interactable_attribs),
	]


# Override
func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("held_item", "potato")]


# Override
func get_requirements() -> Array[RequirementSpec]:
	var reqs: Array[RequirementSpec] = [RequirementSpec.binding_equals("held_item", "potato")]
	if object_location != null:
		reqs.append(RequirementSpec.fact("at_target", [object_location]))
	return reqs


# Override
func get_provisions() -> Array[ProvisionSpec]:
	return [ProvisionSpec.binding("held_item", "cooked_potato"), ProvisionSpec.fact("is_food", [])]


# Override
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


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("held_item", "cooked_potato")


# Override
## Native form of [method simulate_effect]: the raw potato becomes a cooked one.
func get_effect_spec() -> Array:
	return [
		{"op": "set", "target": "agent", "prop": "held_item", "value": "cooked_potato"},
	]


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	set_state(agent, "cook_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	if not is_instance_valid(campfire_ref):
		return Action.Status.FAILURE
	# The fire kept burning while the agent walked here.
	if campfire_ref.current_fuel < min_fuel_to_cook:
		return Action.Status.FAILURE

	var elapsed: float = get_state(agent, "cook_elapsed") + delta
	set_state(agent, "cook_elapsed", elapsed)

	if elapsed >= COOK_DURATION:
		agent.blackboard.set_property("held_item", "cooked_potato")
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	erase_state(agent, "cook_elapsed")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Cook Potato"


# Override
func get_description() -> String:
	return "Cook a raw potato at the campfire (requires minimum fuel level)."
