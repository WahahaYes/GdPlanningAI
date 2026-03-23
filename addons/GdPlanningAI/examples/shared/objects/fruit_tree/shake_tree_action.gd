class_name ShakeTreeAction
extends SpatialAction
## Navigates an agent to a [FruitTreeObject] and shakes it to drop fruit.
##[br]
##[br]
## Demonstrates a validity check that reads external cooldown state from a world object,
## and a simulated effect that fakes hunger gain (the planner cannot model spawning new
## objects, so we substitute the expected minimum hunger restored).


## Duration of the shaking animation in seconds.
const SHAKE_DURATION: float = 0.5

## Reference to the fruit tree this action targets.
var fruit_tree: FruitTreeObject
## Pre-computed hunger gain for planning purposes (minimum fruit × hunger_value).
var _sim_hunger_gain: float


# Override
func _init(
		p_object_location: GdPAILocationData,
		p_interactable_attribs: GdPAIInteractable,
		p_fruit_tree: FruitTreeObject,
) -> void:
	super (p_object_location, p_interactable_attribs)
	fruit_tree = p_fruit_tree

	var fruit: Node = fruit_tree.fruit_prefab.instantiate()
	fruit.queue_free()
	var food_item: FoodObject = GdPAIUTILS.get_child_of_type(fruit, FoodObject)
	_sim_hunger_gain = food_item.hunger_value * fruit_tree.drop_min_amount


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = super ()
	checks.append(Precondition.agent_has_property("hunger"))
	checks.append(Precondition.check_is_object_valid(fruit_tree))
	checks.append(Precondition.agent_property_less_than("hunger", 100.0))
	checks.append(Precondition.custom(
		func(_bb: GdPAIBlackboard, _ws: GdPAIBlackboard) -> bool:
		return not fruit_tree.is_on_cooldown
	))
	return checks


# Override
func get_action_cost(
		agent_blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> float:
	var cost: float = super (agent_blackboard, world_state)
	if cost == INF:
		return INF
	return 100.0 + cost


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
	agent_blackboard.set_property("hunger", hunger + _sim_hunger_gain)


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if super (agent) == Action.Status.FAILURE:
		return Action.Status.FAILURE
	set_state(agent, "shake_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var parent_status: Action.Status = super (agent, delta)
	if parent_status == Action.Status.FAILURE:
		return Action.Status.FAILURE

	if fruit_tree.is_on_cooldown:
		return Action.Status.FAILURE

	if not get_state(agent, "target_reached"):
		return Action.Status.RUNNING

	var shake_elapsed: float = get_state(agent, "shake_elapsed") + delta
	set_state(agent, "shake_elapsed", shake_elapsed)

	if shake_elapsed >= SHAKE_DURATION:
		fruit_tree.drop_fruit()
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	super (agent)
	erase_state(agent, "shake_elapsed")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Shake Tree"


# Override
func get_description() -> String:
	return "Navigate to and shake a fruit tree to drop fruit."
