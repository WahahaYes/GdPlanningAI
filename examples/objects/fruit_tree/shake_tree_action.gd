class_name ShakeTreeAction
extends Action
## An action that shakes a fruit tree to drop fruit.
## This action requires the agent to be at the target location (via at_target fact)
## and does NOT handle navigation itself - that's handled by GoToAction.

## Duration of the shaking animation in seconds.
const SHAKE_DURATION: float = 0.5

## Reference to the fruit tree this action targets.
var fruit_tree: FruitTreeObject
## Pre-computed hunger gain for planning purposes (minimum fruit × hunger_value).
var _sim_hunger_gain: float


# Override
func _init(
	p_fruit_tree: FruitTreeObject,
) -> void:
	fruit_tree = p_fruit_tree

	var fruit: Node = fruit_tree.fruit_prefab.instantiate()
	fruit.queue_free()
	var food_item: FoodObject = GdPAIUTILS.get_child_of_type(fruit, FoodObject)
	_sim_hunger_gain = food_item.hunger_value * fruit_tree.drop_min_amount


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	# Interaction cost is minimal - navigation is handled by GoToAction
	return 100.0


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("hunger"))
	checks.append(Precondition.check_is_object_valid(fruit_tree))
	checks.append(Precondition.agent_property_greater_than("hunger", 0.0))

	var tree_not_on_cooldown = func(_bb: GdPAIBlackboard, _ws: GdPAIBlackboard) -> bool:
		return not fruit_tree.is_on_cooldown

	checks.append(Precondition.custom_with_deps(tree_not_on_cooldown, [fruit_tree]))
	return checks


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func get_requirements() -> Array[RequirementSpec]:
	# Require that the agent is at this fruit tree's location
	if fruit_tree.location_data != null:
		return [RequirementSpec.fact("at_target", [fruit_tree.location_data])]
	return []


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	var hunger: float = agent_blackboard.get_property("hunger")
	agent_blackboard.set_property("hunger", max(0.0, hunger - _sim_hunger_gain))


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Check if the fruit tree is still valid
	if not is_instance_valid(fruit_tree):
		return Action.Status.FAILURE
		
	set_state(agent, "shake_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	# Check if the fruit tree is still valid
	if not is_instance_valid(fruit_tree):
		return Action.Status.FAILURE
	
	if fruit_tree.is_on_cooldown:
		return Action.Status.FAILURE

	var shake_elapsed: float = get_state(agent, "shake_elapsed") + delta
	set_state(agent, "shake_elapsed", shake_elapsed)

	if shake_elapsed >= SHAKE_DURATION:
		fruit_tree.drop_fruit()
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	erase_state(agent, "shake_elapsed")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Shake Tree"


# Override
func get_description() -> String:
	return "Shake a fruit tree to drop fruit (requires GoToAction for navigation)."
