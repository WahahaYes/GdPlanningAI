class_name FoodAction
extends SpatialAction
## Navigates an agent to a [FoodObject] and eats it.
##[br]
##[br]
## Demonstrates [SpatialAction]: extending the base navigation action to add
## domain-specific preconditions, cost, effect, and a timed eating phase.


## Reference to the food item that provided this action.
var food_item: FoodObject


# Override
func _init(
		p_object_location: GdPAILocationData,
		p_interactable_attribs: GdPAIInteractable,
		p_food_item: FoodObject,
) -> void:
	super (p_object_location, p_interactable_attribs)
	food_item = p_food_item


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = super ()
	checks.append(Precondition.agent_has_property("hunger"))
	checks.append(Precondition.check_is_object_valid(food_item))
	checks.append(Precondition.agent_property_less_than("hunger", 100.0))
	return checks


# Override
func get_action_cost(
		agent_blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> float:
	var cost: float = super (agent_blackboard, world_state)
	if cost == INF:
		return INF
	if not is_instance_valid(food_item):
		return INF
	return cost + food_item.eating_duration


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func simulate_effect(
		agent_blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> void:
	super (agent_blackboard, world_state)
	var hunger: float = agent_blackboard.get_property("hunger")
	agent_blackboard.set_property("hunger", hunger + food_item.hunger_value)


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if super (agent) == Action.Status.FAILURE:
		return Action.Status.FAILURE
	set_state(agent, "eating_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var parent_status: Action.Status = super (agent, delta)
	if parent_status == Action.Status.FAILURE:
		return Action.Status.FAILURE

	if not get_state(agent, "target_reached"):
		return Action.Status.RUNNING

	var eating_elapsed: float = get_state(agent, "eating_elapsed") + delta
	set_state(agent, "eating_elapsed", eating_elapsed)

	if eating_elapsed >= food_item.eating_duration:
		var hunger: float = agent.blackboard.get_property("hunger")
		agent.blackboard.set_property("hunger", hunger + food_item.hunger_value)
		food_item.entity.queue_free()
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	super (agent)
	erase_state(agent, "eating_elapsed")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Eat Food"


# Override
func get_description() -> String:
	return "Navigate to and eat a food item."
