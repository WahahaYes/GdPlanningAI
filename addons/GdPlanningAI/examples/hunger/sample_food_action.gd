class_name SampleFoodAction
extends SpatialAction
## Simple GdPAI spatial action to eat food for the GdPAI usage demo.

## Reference to the food item that provided this action.
var food_item: SampleFoodObject


# Override
func _init(
		p_object_location: GdPAILocationData,
		p_interactable_attribs: GdPAIInteractable,
		p_food_item: SampleFoodObject,
) -> void:
	super(p_object_location, p_interactable_attribs)
	self.food_item = p_food_item


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = super()
	checks.append(Precondition.agent_has_property("hunger"))
	checks.append(Precondition.check_is_object_valid(food_item))
	checks.append(Precondition.agent_property_less_than("hunger", 100))
	return checks


# Override
func get_action_cost(
		agent_blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> float:
	var cost: float = super(agent_blackboard, world_state)
	if cost == INF:
		return INF

	if not is_instance_valid(food_item):
		return INF
	# Including how long it takes to eat the item as a cost metric for demo purposes.
	return cost + food_item.eating_duration


# Override
func get_preconditions() -> Array[Precondition]:
	# So long as we can get to the food (handled in validity checks), we can eat it.
	return []


# Override
func simulate_effect(
		agent_blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> void:
	super(agent_blackboard, world_state)
	var hunger: float = agent_blackboard.get_property("hunger")
	hunger += food_item.hunger_value
	agent_blackboard.set_property("hunger", hunger)


# Override
func reverse_simulate_effect(
		_agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> void:
	pass


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if super(agent) == Action.Status.FAILURE:
		return Action.Status.FAILURE
	agent.blackboard.set_property(uid_property("eating_duration"), 0)
	return Action.Status.SUCCESS


# Override
func perform_action(
		agent: GdPAIAgent,
		delta: float,
) -> Action.Status:
	var parent_status: Action.Status = super(agent, delta)
	if parent_status == Action.Status.FAILURE:
		return Action.Status.FAILURE

	if not agent.blackboard.get_property(uid_property("target_reached")):
		return Action.Status.RUNNING

	# Update how long we've been eating the food item.
	var eating_duration: float = agent.blackboard.get_property(uid_property("eating_duration"))
	eating_duration += delta
	agent.blackboard.set_property(uid_property("eating_duration"), eating_duration)

	if eating_duration > food_item.eating_duration:
		# Update hunger and destroy the food.
		var hunger: float = agent.blackboard.get_property("hunger")
		agent.blackboard.set_property("hunger", hunger + food_item.hunger_value)
		food_item.entity.queue_free()
		return Action.Status.SUCCESS
	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	super(agent)
	agent.blackboard.erase_property(uid_property("eating_duration"))
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Eat Food"


# Override
func get_description() -> String:
	return "Eat a food item."
