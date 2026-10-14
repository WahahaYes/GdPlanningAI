class_name DropItemAction
extends Action
## Drops the currently held inventory item without interacting with a world object.
##[br]
##[br]
## Demonstrates an agent-provided self action that exists to unblock replanning when
## a single held-item slot prevents the next desired interaction.[br]
##[br]
## It has no requirements or provisions: nothing needs to precede it and it hands no
## binding forward. Its effect is only to empty [code]held_item[/code], which satisfies
## the [code]held_item == ""[/code] preconditions of pick up and dig. It is registered by
## [CampfireBehaviorConfig] so a carrying agent can abandon its item (for example a potato
## when the fire needs wood) instead of being stuck with a full hand. Nothing is
## spawned into the world: the item is simply discarded.

## How long the drop action should take in seconds.
var drop_duration: float = 0.2


func _init(p_drop_duration: float = 0.2) -> void:
	drop_duration = p_drop_duration


# Override
func get_validity_checks() -> Array[Precondition]:
	# Always offered; the precondition below decides whether it can run.
	var checks: Array[Precondition] = []
	return checks


# Override
func get_preconditions() -> Array[Precondition]:
	return [Precondition.agent_property_not_equal_to("held_item", "")]


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return drop_duration


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("held_item", "")


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	set_state(agent, "drop_elapsed", 0.0)
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var drop_elapsed: float = get_state(agent, "drop_elapsed") + delta
	set_state(agent, "drop_elapsed", drop_elapsed)

	if drop_elapsed >= drop_duration:
		agent.blackboard.set_property("held_item", "")
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	erase_state(agent, "drop_elapsed")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Drop Item"


# Override
func get_description() -> String:
	return "Drop the item currently being held."
