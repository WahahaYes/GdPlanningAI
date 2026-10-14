class_name Action
extends RefCounted
## Base class for everything an agent can do. Extend it to write a new action.[br]
## [b]Declarative hooks[/b] ([code]get_*[/code]) describe the action to the planner; they are read
## once per plan request.[br]
## [b]Planning hooks[/b] ([method get_action_cost], [method simulate_effect]) run on simulated
## blackboards: no scene tree access, no [code]await[/code].[br]
## [b]Execution hooks[/b] ([code]*_perform_action[/code]) run the chosen plan on the main thread.[br]
## See [code]docs/AUTHORING_GUIDE.md[/code] for timing rules and [code]examples/objects/[/code] for real actions.

## Result of [method pre_perform_action], [method perform_action] and
## [method post_perform_action].
enum Status {
	FAILURE,  ## Abort the plan; the agent replans.
	RUNNING,  ## Not finished; called again next frame.
	SUCCESS,  ## Done; the next action starts.
}

## Index of this action in the executing plan, or [code]-1[/code]. Set by the agent; keys
## [method set_state] data.
var chain_position: int = -1


## Returns hard gates checked once against the initial state. A failing check drops the action
## from the search; unlike [method get_preconditions], the planner never tries to satisfy them.
func get_validity_checks() -> Array[Precondition]:
	return []


# Override
## Returns the cost of this action in the simulated state; [code]INF[/code] rules it out.
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0


## Returns conditions the planner satisfies by chaining earlier actions whose
## [method simulate_effect] makes them true.
func get_preconditions() -> Array[Precondition]:
	return []


## Returns symbolic needs matched by name against earlier actions' [method get_provisions],
## e.g. [code]RequirementSpec.fact("at_target", [location])[/code] pulls in a [GoToAction].
func get_requirements() -> Array[RequirementSpec]:
	return []


## Returns the facts and bindings this action offers to later actions' requirements.
func get_provisions() -> Array[ProvisionSpec]:
	return []


# Override
## Applies this action's effect to the simulated blackboards in place. Must match what
## [method perform_action] really does: the planner trusts it.
func simulate_effect(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	pass


# Override
## Called once on every action in a new plan, in order, before the first action runs. Validate
## the live scene and initialise [method set_state] data; [constant Status.FAILURE] aborts the plan.
func pre_perform_action(_agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


# Override
## Called every frame while this action is current. Return [constant Status.RUNNING] to continue.
## Stop anything this action started before returning: [method post_perform_action] only runs when
## the whole plan ends.
func perform_action(
	_agent: GdPAIAgent,
	_delta: float,
) -> Status:
	return Status.SUCCESS


# Override
## Called on every action in the chain when the plan completes or fails, even ones that never
## ran. Erase [method set_state] data here. Not called when a newer plan replaces this one.
func post_perform_action(_agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


## Stores a per-run value, keyed by this instance and its [member chain_position].
func set_state(agent: GdPAIAgent, key: String, value: Variant) -> void:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	agent.blackboard.set_property(str(get_instance_id()) + "_" + pos + "_" + key, value)


## Returns the value stored by [method set_state], or [code]null[/code].
func get_state(agent: GdPAIAgent, key: String) -> Variant:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	return agent.blackboard.get_property(str(get_instance_id()) + "_" + pos + "_" + key)


## Removes a value stored by [method set_state].
func erase_state(agent: GdPAIAgent, key: String) -> void:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	agent.blackboard.erase_property(str(get_instance_id()) + "_" + pos + "_" + key)


## Returns [code]true[/code] if [method set_state] stored a value for [param key].
func has_state(agent: GdPAIAgent, key: String) -> bool:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	return agent.blackboard.has_property(str(get_instance_id()) + "_" + pos + "_" + key)


## Returns the instance used for one plan step. Override to return a copy when the action keeps
## mutable members (as [GoToAction] does); the default shares [code]self[/code].
func clone_for_plan() -> Action:
	return self


## Returns a short, stable name used in logs and sent to the engine.
func get_title() -> String:
	return ""


## Returns a longer description for debug output.
func get_description() -> String:
	return "Root action."
