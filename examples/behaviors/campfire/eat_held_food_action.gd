class_name EatHeldFoodAction
extends Action
## Consumes an item already held in the agent's inventory slot.
##[br]
##[br]
## Demonstrates an agent-provided self action whose validity is determined entirely
## from blackboard state rather than a spatial world target.

## Maps held item ids to the hunger amount they restore when eaten.
var hunger_restored_by_item: Dictionary = {}
## How long the eating action should take in seconds.
var eat_duration: float = 1.5
## Hunger restored during planning before a held item provider has been bound.
var optimistic_unbound_restore: float = 20.0


func _init(
	p_hunger_restored_by_item: Dictionary = {},
	p_eat_duration: float = 1.5,
	p_optimistic_unbound_restore: float = 20.0,
) -> void:
	hunger_restored_by_item = p_hunger_restored_by_item.duplicate(true)
	eat_duration = p_eat_duration
	optimistic_unbound_restore = p_optimistic_unbound_restore


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("hunger"))
	return checks


# Override
func get_preconditions() -> Array[Precondition]:
	var preconds: Array[Precondition] = [Precondition.agent_property_greater_than("hunger", 0.0)]
	
	# If we are already holding something, it must be a food item we know how to eat.
	# We use a custom check for this since it depends on our internal map.
	preconds.append(Precondition.custom(self , "_is_holding_food"))
	
	return preconds


# Override
func get_requirements() -> Array[RequirementSpec]:
	# Require that held_item binding exists AND that it is marked as food.
	# This prevents the planner from trying to 'eat' non-food items like wood.
	return [
		RequirementSpec.binding_exists("held_item"),
		RequirementSpec.fact("is_food", [])
	]


func _is_holding_food(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> bool:
	var item = agent.get_property("held_item")
	if item == null or str(item).is_empty():
		# Not holding anything - this precondition is satisfied because 
		# we expect a later action in the chain to provide the food.
		return true
	
	return hunger_restored_by_item.has(str(item))


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
	var held_item = agent_blackboard.get_property("held_item")
	if hunger == null:
		return

	var hunger_restored: float
	var held_item_id: String = String(held_item) if held_item != null else ""

	# Durante backward planning, this action may be considered before its
	# held_item requirement has been satisfied. Use a conservative optimistic
	# restore so the planner can recognize the action as relevant, then rely on
	# get_requirements() and forward validation to require a real provided item.
	if held_item_id.is_empty():
		hunger_restored = optimistic_unbound_restore
	elif not hunger_restored_by_item.has(held_item_id):
		return
	else:
		hunger_restored = float(hunger_restored_by_item[held_item_id])
	var new_hunger = max(0.0, float(hunger) - hunger_restored)
	agent_blackboard.set_property("hunger", new_hunger)
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
	var held_item_id: String = String(held_item)
	if not hunger_restored_by_item.has(held_item_id):
		return Action.Status.FAILURE

	var eat_elapsed: float = get_state(agent, "eat_elapsed") + delta
	set_state(agent, "eat_elapsed", eat_elapsed)

	if eat_elapsed >= eat_duration:
		var hunger_restored: float = float(hunger_restored_by_item[held_item_id])
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
