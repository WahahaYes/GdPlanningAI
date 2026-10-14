class_name CookPotatoAction
extends Action
## Cooks a held raw potato at a campfire, turning it into a cooked potato.
##[br]
##[br]
## The middle of the cook-and-eat chain: Go To (potato), Dig Potato, Go To (campfire),
## Cook Potato, Eat Held Food. It transforms the held item rather than consuming it:
## it requires [code]held_item == "potato"[/code] and provides
## [code]held_item == "cooked_potato"[/code] plus [code]is_food[/code], which is what lets
## [EatHeldFoodAction] follow. Digging also provides [code]is_food[/code], but the eat
## action's own precondition only accepts items in its restore table, and that table
## lists [code]cooked_potato[/code] (50 hunger) and not [code]potato[/code]. A raw potato
## therefore cannot be eaten, which is why cooking is not optional.[br]
##[br]
## This action is where the two goals meet. Its cost is [constant INF] when the fire is
## below [member min_fuel_to_cook], so the planner cannot choose a cook-and-eat plan
## for a cold fire. [MaintainFireGoal] keeps the fire above that line, and the
## real cook also checks the fuel at execution time and fails (forcing a replan) if the
## fire has since burned low.

## Seconds the agent spends cooking. Also the planning cost when the fire is hot enough.
const COOK_DURATION: float = 2.0

## The campfire the potato is cooked on; its fuel gates the action.
var campfire_ref: CampfireObject
## The campfire's location; named in the [code]at_target[/code] requirement.
var object_location: GdPAILocationData
## The campfire's interactable component, kept for parity with the other object actions.
var interactable_attribs: GdPAIInteractable
## Fuel the fire must have for cooking to be allowed.
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
	# Static checks: the action is withdrawn if the campfire or its components were freed.
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
	# Replaces the held potato with a cooked one and marks it edible for EatHeldFoodAction.
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
	# A cold fire makes cooking impossible, so no plan can include it.
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
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	set_state(agent, "cook_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	if not is_instance_valid(campfire_ref):
		return Action.Status.FAILURE
	# The plan assumed a hot fire, but the fire kept burning while the agent walked here.
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
