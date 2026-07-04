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
func get_requirements() -> Array[RequirementSpec]:
	# Return symbolic dependencies that must be satisfied by predecessor action provisions.
	# Used by the backward-chaining planner to discover which earlier actions enable this one.
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	# Return bindings and facts this action makes available to later actions in the chain.
	# Provisions are injected into the agent blackboard of the consumer action.
	# They do not affect world state.
	return []


# Override
func clone_for_plan() -> Action:
	# Return a clean copy of this action for use in planning.
	# The scheduler clones actions when they appear multiple times in a plan with different bindings.
	# Override this if your action stores mutable state that must be isolated per occurrence.
	return self


# Override
func simulate_effect(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	# Mutate the passed blackboards in-place. The return value is ignored.
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
