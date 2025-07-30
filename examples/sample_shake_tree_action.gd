## Simple GdPAI spatial action to shake the tree (in 2D) for the GdPAI usage demo.
class_name SampleShakeTreeAction
extends SpatialAction

## While not necessary, I'm encoding a duration into shaking the tree here, just to make the
## simulation a little more visually interesting.
const SHAKE_DURATION: float = 0.5
## Reference to the food item that provided this action.
var fruit_tree: SampleFruitTreeObject
## An internal param to cache how much food the tree is guaranteed to drop.
var _fruit_hunger_value: float


# Override
func _init(
	object_location: GdPAILocationData,
	interactable_attribs: GdPAIInteractable,
	fruit_tree: SampleFruitTreeObject
):
	# If extending _init(), make sure to call super() so a uid is created and references are
	# assigned.
	super(object_location, interactable_attribs)
	self.fruit_tree = fruit_tree

	# I am faking that the agent will restore hunger by shaking the tree.  It doesn't
	# actually do that; it drops fruit.  Faking it makes it easier to hook preconditions, rather
	# than trying to simulate the creation of new objects.

	# Create a temporary instance for the tree's fruit to determine hunger restored.
	var fruit = fruit_tree.fruit_prefab.instantiate()
	fruit.queue_free()
	var food_item: SampleFoodObject = GdPAIUTILS.get_child_of_type(fruit, SampleFoodObject)
	_fruit_hunger_value = food_item.hunger_value * fruit_tree.drop_min_amount


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = super()
	# Add any additional checks here.

	checks.append(Precondition.agent_has_property("hunger"))
	checks.append(Precondition.check_is_object_valid(fruit_tree))

	var is_hungry_check: Precondition = Precondition.new()
	is_hungry_check.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return blackboard.get_property("hunger") < 100
	checks.append(is_hungry_check)

	var on_cooldown_check: Precondition = Precondition.new()
	on_cooldown_check.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		# Tree shouldn't have recently been shaken.
		return not fruit_tree.is_on_cooldown
	checks.append(on_cooldown_check)

	return checks


# Override
func get_action_cost(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> float:
	var cost: float = super(agent_blackboard, world_state)
	if cost == INF:
		return INF
	# Add any additional cost computations.

	# This action needs a high cost to discourage shaking the tree when there are alternatives.
	return 100 + cost


# Override
func get_preconditions() -> Array[Precondition]:
	# So long as we can get to the tree (handled in validity checks), we can shake it.
	return []


# Override
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	super(agent_blackboard, world_state)
	# Add any additional simulation here.

	var hunger: float = agent_blackboard.get_property("hunger")
	agent_blackboard.set_property("hunger", hunger + _fruit_hunger_value)


# Override
func reverse_simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	pass


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if super(agent) == Action.Status.FAILURE:
		return Action.Status.FAILURE
	# Add any additional preactions here.
	agent.blackboard.set_property(uid_property("shake_duration"), 0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var parent_status: Action.Status = super(agent, delta)
	if parent_status == Action.Status.FAILURE:
		return Action.Status.FAILURE

	# Check that the tree has not been shaken by another agent.
	if fruit_tree.is_on_cooldown:
		return Action.Status.FAILURE

	if not agent.blackboard.get_property(uid_property("target_reached")):
		# Can add any actions that occur while navigating here.

		return Action.Status.RUNNING
	else:
		# Add the main action that occurs after the agent navigates to the object here.

		# Update how long we've been eating the food item.
		var shake_duration: float = agent.blackboard.get_property(uid_property("shake_duration"))
		shake_duration += delta
		agent.blackboard.set_property(uid_property("shake_duration"), shake_duration)

		if shake_duration > SHAKE_DURATION:
			# spawn the food.
			fruit_tree.drop_fruit()
			return Action.Status.SUCCESS
		else:
			return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	super(agent)
	# Add any additional postactions here.
	agent.blackboard.erase_property(uid_property("shake_duration"))
	return Action.Status.SUCCESS
