# meta-description: GdPAI Action template.
# meta-default: true
extends Action

# Planning hooks (cost, simulate_effect) run on simulated blackboards: no scene tree, no await.
# Execution hooks (*_perform_action) run the real action on the main thread.
# Keep per-run data in set_state()/get_state(), not member variables.


# Override
func get_title() -> String:
	return ""


# Override
func get_description() -> String:
	return ""


# Override
func get_validity_checks() -> Array[Precondition]:
	# Gates checked once against the initial state; a failing check drops the action.
	# e.g. Precondition.check_is_object_valid(target)
	return []


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	# Lower is preferred. Return INF to rule the action out in this state.
	return 0


# Override
func get_preconditions() -> Array[Precondition]:
	# Must hold in the simulated state before this action; the planner chains in earlier
	# actions to make them true. Prefer built-in preconditions.
	return []


# Override
func get_requirements() -> Array[RequirementSpec]:
	# What an earlier action must provide, e.g. RequirementSpec.fact("at_target", [location])
	# to have a GoToAction planned first.
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	# Bindings and facts this action offers later actions' requirements.
	return []


# Override
func clone_for_plan() -> Action:
	# Return a copy if the action keeps mutable members; one instance may appear several times.
	return self


# Override
func simulate_effect(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	# Change the blackboards in place to match what perform_action really does.
	pass


# Override
func pre_perform_action(_agent: GdPAIAgent) -> Action.Status:
	# Runs for every action when the plan starts, before the first one runs.
	# Validate the live scene and set up set_state() data. FAILURE aborts the plan.
	return Action.Status.SUCCESS


# Override
func perform_action(
	_agent: GdPAIAgent,
	_delta: float,
) -> Action.Status:
	# Called every frame while current: RUNNING to continue, SUCCESS to advance, FAILURE to abort.
	# Stop anything this action started before returning SUCCESS or FAILURE.
	return Action.Status.SUCCESS


# Override
func post_perform_action(_agent: GdPAIAgent) -> Action.Status:
	# Runs for every action only when the whole plan ends, even ones that never ran.
	# Erase set_state() data here.
	return Action.Status.SUCCESS
