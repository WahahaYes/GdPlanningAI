class_name AddFuelAction
extends Action
## Adds held wood to a campfire. Last step of the maintain-fire chain: Go To (pile), Pick Up
## Wood, Go To (fire), Add Fuel.[br]
## The [code]held_item[/code] need appears twice on purpose: the requirements tell the planner
## which earlier actions to chain in, and the precondition checks the simulated state. Cost is
## [constant INF] when the fire is full, so agents never carry wood to it.

## Seconds spent adding the wood; also the planning cost.
const ADD_FUEL_DURATION: float = 1.0

var campfire_ref: CampfireObject
## The campfire's location, named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable
var fuel_per_wood: float


func _init(
	p_campfire: CampfireObject,
	p_object_location: GdPAILocationData,
	p_interactable_attribs: GdPAIInteractable,
	p_fuel_per_wood: float,
) -> void:
	campfire_ref = p_campfire
	object_location = p_object_location
	interactable_attribs = p_interactable_attribs
	fuel_per_wood = p_fuel_per_wood


# Override
func get_validity_checks() -> Array[Precondition]:
	return []


# Override
func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_equal_to("held_item", "wood")]


# Override
func get_requirements() -> Array[RequirementSpec]:
	var reqs: Array[RequirementSpec] = [RequirementSpec.binding_equals("held_item", "wood")]
	if object_location != null:
		reqs.append(RequirementSpec.fact("at_target", [object_location]))
	return reqs


# Override
func get_provisions() -> Array[ProvisionSpec]:
	return []


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
	if current_fuel == null or float(current_fuel) >= 100.0:
		return INF
	return ADD_FUEL_DURATION


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> void:
	var campfire: SimObjectProxy = world_state.get_object_for(campfire_ref)
	if campfire != null:
		var current_fuel: Variant = campfire.get_property("current_fuel")
		if current_fuel != null:
			campfire.set_property("current_fuel", min(100.0, float(current_fuel) + fuel_per_wood))

	agent_blackboard.set_property("held_item", "")


# Override
## Native form of [method simulate_effect]: pours fuel (capped at 100) and frees the hand.
## Empty while the campfire is gone, so the planner falls back to the callable.
func get_effect_spec() -> Array:
	if not is_instance_valid(campfire_ref):
		return []
	var campfire_uid: String = str(campfire_ref.get_instance_id())
	return [
		{
			"op": "inc",
			"target": "object",
			"uid": campfire_uid,
			"prop": "current_fuel",
			"amount": fuel_per_wood,
		},
		{
			"op": "clamp",
			"target": "object",
			"uid": campfire_uid,
			"prop": "current_fuel",
			"hi": 100.0,
		},
		{"op": "set", "target": "agent", "prop": "held_item", "value": ""},
	]


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	set_state(agent, "add_fuel_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	if not is_instance_valid(campfire_ref):
		return Action.Status.FAILURE

	var elapsed: float = get_state(agent, "add_fuel_elapsed") + delta
	set_state(agent, "add_fuel_elapsed", elapsed)

	if elapsed >= ADD_FUEL_DURATION:
		campfire_ref.current_fuel = min(100.0, campfire_ref.current_fuel + fuel_per_wood)
		agent.blackboard.set_property("held_item", "")
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	erase_state(agent, "add_fuel_elapsed")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Add Fuel"


# Override
func get_description() -> String:
	return "Add wood to the campfire to increase its fuel level."
