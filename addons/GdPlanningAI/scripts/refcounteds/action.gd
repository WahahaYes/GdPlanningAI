class_name Action
extends RefCounted
## This is a base class to extend with actual AI actions.  During planning, actions are simulated
## instantaneously.  After planning, actions are carried out by the agent in real time.

## Return states for actions during true simulation.
enum Status { FAILURE, RUNNING, SUCCESS }


## List of static preconditions needed for the action to be considered.  This is
## evaluated at the time of assigning wordly actions (so, there is no need to grab sim data).
func get_validity_checks() -> Array[Precondition]:
	return []


## Computes the cost to complete this action.  Called during Rust planning simulation; both
## [param _agent_blackboard] and [param _world_state] are [GdPAIBlackboard] objects backed by Rust.
##[br]
##[br]
## To access a simulated object, call [code]world_state.get_object_for(my_object_data_node)[/code],
## which returns a [SimObjectProxy] snapshot.  Read and write its state with
## [code]get_property[/code] / [code]set_property[/code], or the [code]position[/code] shorthand.
##[br]
##[br]
## Validity checks are guaranteed true at this point, EXCEPT live scene-tree references which may
## have been freed.  If a reference is invalid, return [code]INF[/code] to skip this action.
##[br]
##[br]
## [b]Do not use [code]await[/code] and do not access the scene tree from this method.[/b]


func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0


## Lists the preconditions necessary for this action to be carried out.  Evaluated during Rust
## planning simulation against simulated [GdPAIBlackboard] state.
##[br]
##[br]
## Preconditions are checked against the simulated blackboards, not the live scene tree.


func get_preconditions() -> Array[Precondition]:
	return []


## Lists planner-readable requirements that should be satisfied earlier in the action chain.


func get_requirements() -> Array[RequirementSpec]:
	return []


## Lists planner-readable provisions this action contributes for later actions.


func get_provisions() -> Array[ProvisionSpec]:
	return []


## Simulates the effect of this action onto the agent and world blackboards.  Modifies both
## in-place; does not return a value.
##[br]
##[br]
## Both [param _agent_blackboard] and [param _world_state] are Rust-backed [GdPAIBlackboard]
## instances.  To access a simulated object, call
## [code]world_state.get_object_for(my_object_data_node)[/code], which returns a [SimObjectProxy]
## snapshot.  Writes to [code]set_property[/code] or [code]position[/code] on the proxy are
## reflected back into the simulation state immediately.
##[br]
##[br]
## [b]Do not use [code]await[/code] and do not access the scene tree from this method.[/b]


func simulate_effect(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	pass


## Perform any pre computations for the action.  All actions' pre_perform methods are called at
## the start of an action chain, regardless of if the plan succeeds or not.  Can return
## Status.FAILURE to indicate the plan should be aborted.
##[br]
##[br]
## At this point, validity checks true during planning could be false in the real world.


func pre_perform_action(_agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


## Perform the action in the actual simulation.  Can return RUNNING for actions that have a
## duration.
##[br]
##[br]
## Need to monitor any validity checks that could become false after some time.


func perform_action(
	_agent: GdPAIAgent,
	_delta: float,
) -> Status:
	return Status.SUCCESS


## Perform any post computations for the action.  Status currently doesn't matter because the plan
## is already done.  All actions' post_perform methods are called, regardless of if the plan
## succeeded.  These methods should make sure to safely de-allocate anything created for the action.


func post_perform_action(_agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


## Sets a state variable for the action.  The key is internally prefixed with the action's
## instance id to avoid collisions.


func set_state(agent: GdPAIAgent, key: String, value: Variant) -> void:
	agent.blackboard.set_property(str(get_instance_id()) + "_" + key, value)


## Gets a state variable for the action.


func get_state(agent: GdPAIAgent, key: String) -> Variant:
	return agent.blackboard.get_property(str(get_instance_id()) + "_" + key)


## Erases a state variable for the action.


func erase_state(agent: GdPAIAgent, key: String) -> void:
	agent.blackboard.erase_property(str(get_instance_id()) + "_" + key)


## Checks if a state variable exists for the action.


func has_state(agent: GdPAIAgent, key: String) -> bool:
	return agent.blackboard.has_property(str(get_instance_id()) + "_" + key)


## Returns a short title for the action.


func get_title() -> String:
	return ""


## Returns a description of the action.


func get_description() -> String:
	return "Root action."
