class_name AddFuelAction
extends Action
## Adds a held piece of wood to a campfire, raising its fuel.
##[br]
##[br]
## The last link of the maintain-fire chain: Go To (wood pile), Pick Up Wood, Go To
## (campfire), Add Fuel. It is provided by [CampfireObject], so one instance exists per
## fire and targets that fire.[br]
##[br]
## It states the same need two ways, and both are intentional. The
## [b]precondition[/b] [code]held_item == "wood"[/code] is checked against the simulated
## agent at this step. The [b]requirements[/b] (a [code]held_item[/code] binding of
## wood plus [code]at_target[/code] for this campfire) tell the planner which earlier
## actions to search for: [PickUpWoodAction] provides the binding, [GoToAction] the
## location fact. Without the requirements the planner would not know to schedule
## either; without the precondition the simulation would not check the real state.[br]
##[br]
## Its cost is [constant INF] when the fire is already full (or the fire has been freed),
## which removes the action from consideration. That is how the example keeps agents
## from carrying wood to a fire that cannot take more, with no extra goal logic.

## Seconds the agent spends adding the wood. Also the planning cost.
const ADD_FUEL_DURATION: float = 1.0

## The campfire this action feeds; read for its fuel both when costing and performing.
var campfire_ref: CampfireObject
## The campfire's location; named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
## The campfire's interactable component, kept for parity with the other object actions.
var interactable_attribs: GdPAIInteractable
## Fuel gained per wood item, copied from [member CampfireObject.fuel_per_wood].
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
	# Nothing to check statically; a freed campfire is handled by returning INF from
	# get_action_cost, which is where live references are meant to be tested.
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
	# The effect (more fuel) is a change to the world, which the goal reads directly from
	# the campfire, so there is nothing to hand to a later action.
	return []


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> float:
	if not is_instance_valid(campfire_ref):
		push_warning("[DEBUG] AddFuelAction.get_action_cost: campfire_ref is invalid")
		return INF
	var campfire: SimObjectProxy = world_state.get_object_for(campfire_ref)
	if campfire == null:
		push_warning(
			(
				"[DEBUG] AddFuelAction.get_action_cost: campfire proxy is null for UID "
				+ str(campfire_ref.instance_id())
			)
		)
		return INF
	var current_fuel: Variant = campfire.get_property("current_fuel")
	# A full fire makes the action infinitely expensive so it is never planned.
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

	# Action-Led Hypothetical Progress:
	# This action has a symbolic requirement for 'held_item' == 'wood'.
	# During the Discovery phase of backward planning, the requirement may not
	# yet be physically satisfied in the blackboard. We report the effect
	# anyway so the planner recognizes this action as a candidate for goals
	# that depend on campfire fuel.
	agent_blackboard.set_property("held_item", "")


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	# The timer lives in set_state so each plan step keeps its own progress.
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
