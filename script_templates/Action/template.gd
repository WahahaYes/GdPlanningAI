# meta-description: GdPAI Action template.
# meta-default: true
extends Action


# Override
func get_validity_checks() -> Array[Precondition]:
	return []


# Override
func get_action_cost(
		_agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> float:
	return 0


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func simulate_effect(
		_agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> void:
	pass


# Override
func reverse_simulate_effect(
		_agent_blackboard: GdPAIBlackboard,
		_world_state: GdPAIBlackboard,
) -> void:
	pass


# Override
func pre_perform_action(_agent: GdPAIAgent) -> Action.Status:
	return Action.Status.SUCCESS


# Override
func perform_action(
		_agent: GdPAIAgent,
		_delta: float,
) -> Action.Status:
	return Action.Status.SUCCESS


# Override
func post_perform_action(_agent: GdPAIAgent) -> Action.Status:
	return Action.Status.SUCCESS
