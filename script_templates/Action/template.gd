# meta-description: GdPAI Action template.
# meta-default: true
extends Action


# Override
func get_title() -> String:
	return ""


# Override
func get_description() -> String:
	return ""


# Override
func get_validity_checks() -> Array[Precondition]:
	# Return conditions that must be true for this action to be considered at all.
	return []


# Override
func get_action_cost(
		_agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> float:
	# Return the cost of performing this action. Lower cost is preferred.
	return 0


# Override
func get_preconditions() -> Array[Precondition]:
	# Return conditions that must hold in the simulated state for this action to apply.
	return []


# Override
func simulate_effect(
		_agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> void:
	# Modify the simulated blackboard to reflect what this action does.
	pass


# Override
func pre_perform_action(_agent: GdPAIAgent) -> Action.Status:
	# One-time setup before the action starts. Return FAILURE to abort the plan.
	return Action.Status.SUCCESS


# Override
func perform_action(
		_agent: GdPAIAgent,
		_delta: float,
) -> Action.Status:
	# Called every frame while the action is active.
	# Return RUNNING to continue, SUCCESS to advance, FAILURE to abort.
	return Action.Status.SUCCESS


# Override
func post_perform_action(_agent: GdPAIAgent) -> Action.Status:
	# Cleanup after the action finishes (success or failure).
	return Action.Status.SUCCESS
