class_name ShakeTreeAction
extends Action
## An action that shakes a fruit tree to drop fruit.
## This action requires the agent to be at the target location (via at_target fact)
## and does NOT handle navigation itself - that's handled by GoToAction.[br]
##[br]
## This action shows a planning shortcut: the [i]simulated[/i] effect and the [i]real[/i]
## effect differ on purpose. In simulation it lowers hunger by the fruit it expects to
## drop, so the planner sees Go To then Shake Tree as a way to satisfy [HungerGoal]
## without modelling the fruit that will fall. In the real world shaking only spawns
## fruit; the agent's hunger is unchanged, and the next plan picks them up and eats them.
## That is acceptable here because plans are short and the agent replans constantly.[br]
##[br]
## Its cost of 10 is high on purpose. A banana already lying on the ground costs about 1 to
## pick up, so whenever real food is reachable the planner prefers it and only shakes a
## tree when nothing else is available. The conditions that depend on live scene state
## (hunger above zero, the tree valid, the tree off cooldown) are validity checks, so
## a tree that cannot be shaken is withdrawn rather than planned and then failed.

## Duration of the shaking animation in seconds.
const SHAKE_DURATION: float = 0.5

## Reference to the fruit tree this action targets.
var fruit_tree: FruitTreeObject
## Pre-computed hunger gain for planning purposes (minimum fruit × hunger_value).
var _sim_hunger_gain: float
## Planning cost for shaking the tree. Intentionally high to discourage the planner
## from choosing this over real food items that are already available in the world.
var shake_cost: float = 10.0


# Override
func _init(
	p_fruit_tree: FruitTreeObject,
) -> void:
	fruit_tree = p_fruit_tree

	var fruit: Node = fruit_tree.fruit_prefab.instantiate()
	fruit.queue_free()
	var food_item: FoodObject = GdPAIUTILS.get_child_of_type(fruit, FoodObject)
	if food_item == null:
		push_error("ShakeTreeAction: fruit prefab has no FoodObject child")
		_sim_hunger_gain = 0.0
	else:
		_sim_hunger_gain = food_item.hunger_value * fruit_tree.drop_min_amount


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return shake_cost


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("hunger"))
	checks.append(Precondition.check_is_object_valid(fruit_tree))
	checks.append(Precondition.agent_property_greater_than("hunger", 0.0))

	# The cooldown is plain scene state, so it is tested with a custom check that reads
	# the live node. check_is_object_valid first guarantees the node is still there.
	var tree_not_on_cooldown = func(_bb: GdPAIBlackboard, _ws: GdPAIBlackboard) -> bool:
		if is_instance_valid(fruit_tree):
			return not fruit_tree.is_on_cooldown
		return false

	checks.append(Precondition.custom(tree_not_on_cooldown))
	return checks


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func get_requirements() -> Array[RequirementSpec]:
	# Require that the agent is at this fruit tree's location. GoToAction's wildcard
	# at_target fact satisfies this for any tree, with the target bound at plan time.
	if fruit_tree.location_data != null:
		return [RequirementSpec.fact("at_target", [fruit_tree.location_data])]
	return []


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	# Optimistically report hunger reduction during discovery so the planner
	# picks this action as a candidate for the Hunger goal.
	var hunger = agent_blackboard.get_property("hunger")
	if hunger != null:
		agent_blackboard.set_property("hunger", max(0.0, float(hunger) - _sim_hunger_gain))


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Check if the fruit tree is still valid
	if not is_instance_valid(fruit_tree):
		return Action.Status.FAILURE

	# Per-run timer kept in set_state so it is keyed by agent and plan step.
	set_state(agent, "shake_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	# Check if the fruit tree is still valid
	if not is_instance_valid(fruit_tree):
		return Action.Status.FAILURE

	# Another agent may have shaken the tree since this plan was made.
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
