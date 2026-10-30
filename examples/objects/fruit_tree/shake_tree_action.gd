class_name ShakeTreeAction
extends Action
## Shakes a fruit tree to drop fruit. Navigation is GoTo's job, via [code]at_target[/code].[br]
## The simulated effect lowers hunger by the fruit it expects, so the planner sees shaking as a
## way to eat; in reality it only drops fruit, and the next plan eats them. Its high cost
## makes the planner prefer food already on the ground.

## Duration of the shaking animation in seconds.
const SHAKE_DURATION: float = 0.5

## Reference to the fruit tree this action targets.
var fruit_tree: FruitTreeObject
## Planning cost; high so real food is preferred.
var shake_cost: float = 10.0
## Hunger the simulation assumes a shake restores (minimum drop x [member FoodObject.hunger_value]).
var _sim_hunger_gain: float


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

	# The cooldown is live scene state, so a custom check reads the node.
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
	if fruit_tree.location_data != null:
		return [RequirementSpec.fact("at_target", [fruit_tree.location_data])]
	return []


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	# Optimistic: lets the planner use shaking for the hunger goal.
	var hunger = agent_blackboard.get_property("hunger")
	if hunger != null:
		agent_blackboard.set_property("hunger", max(0.0, float(hunger) - _sim_hunger_gain))


# Override
## Native form of [method simulate_effect]: the optimistic hunger drop, floored at zero.
func get_effect_spec() -> Array:
	return [
		{"op": "dec", "target": "agent", "prop": "hunger", "amount": _sim_hunger_gain},
		{"op": "clamp", "target": "agent", "prop": "hunger", "lo": 0.0},
	]


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if not is_instance_valid(fruit_tree):
		return Action.Status.FAILURE

	set_state(agent, "shake_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
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
