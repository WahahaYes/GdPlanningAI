class_name EatHeldFoodAction
extends Action
## Consumes an item already held in the agent's inventory slot.
##[br]
##[br]
## Demonstrates an agent-provided self action whose validity is determined entirely
## from blackboard state rather than a spatial world target.


## Inventory item ids that this action is allowed to consume.
var allowed_items: Array[String] = []
## How much hunger should be reduced when the held food is eaten.
var hunger_restored: float = 50.0
## How long the eating action should take in seconds.
var eat_duration: float = 1.5


func _init(
		p_allowed_items: Array[String] = [],
		p_hunger_restored: float = 50.0,
		p_eat_duration: float = 1.5,
) -> void:
	allowed_items = p_allowed_items.duplicate()
	hunger_restored = p_hunger_restored
	eat_duration = p_eat_duration


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("held_item"))
	checks.append(Precondition.agent_has_property("hunger"))
	return checks


# Override
func get_preconditions() -> Array[Precondition]:
	var can_eat_held_item = func(
			blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard,
	) -> bool:
		var held_item = blackboard.get_property("held_item")
		var hunger = blackboard.get_property("hunger")
		if held_item == null or hunger == null:
			return false
		if not (held_item is String or held_item is StringName):
			return false
		if float(hunger) <= 0.0:
			return false
		return allowed_items.has(String(held_item))
	return [Precondition.custom(can_eat_held_item)]


# Override
func get_action_cost(
		_agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> float:
	return eat_duration


# Override
func simulate_effect(
		agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> void:
	var hunger = agent_blackboard.get_property("hunger")
	if hunger == null:
		return
	agent_blackboard.set_property("hunger", max(0.0, float(hunger) - hunger_restored))
	agent_blackboard.set_property("held_item", "")


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	set_state(agent, "eat_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var held_item = agent.blackboard.get_property("held_item")
	var hunger = agent.blackboard.get_property("hunger")
	if held_item == null or hunger == null:
		return Action.Status.FAILURE
	if not (held_item is String or held_item is StringName):
		return Action.Status.FAILURE
	if not allowed_items.has(String(held_item)):
		return Action.Status.FAILURE

	var eat_elapsed: float = get_state(agent, "eat_elapsed") + delta
	set_state(agent, "eat_elapsed", eat_elapsed)

	if eat_elapsed >= eat_duration:
		agent.blackboard.set_property("hunger", max(0.0, float(hunger) - hunger_restored))
		agent.blackboard.set_property("held_item", "")
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	erase_state(agent, "eat_elapsed")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Eat Held Food"


# Override
func get_description() -> String:
	return "Consume a held food item that is allowed by this action."
