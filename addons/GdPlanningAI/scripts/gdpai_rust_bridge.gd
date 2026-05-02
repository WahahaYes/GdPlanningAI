class_name GdPAIRustBridge
extends RefCounted

## Internal bridge between GDScript and the Rust planning engine.
##
## Serialises [Action] and [Goal] objects into the plain-dictionary format
## consumed by [GdPAIPlanScheduler.submit_plan]. The Rust layer calls back
## into GDScript via [Callable] references embedded in those dictionaries to
## evaluate costs, effects, and preconditions without copying game state
## across the language boundary.
##
## Not intended for direct use — [GdPAIAgent] owns and calls this internally.


## Resolves the [code]action_chain[/code] indices in [param result] back to
## the original [Action] objects from [param actions].
##
## Returns an ordered [Array[Action]] ready for execution.
func deserialize_plan_result(result: Dictionary, actions: Array[Action]) -> Array[Action]:
	var action_chain: Array[Action] = []
	for action_index in result.action_chain:
		action_chain.append(actions[action_index])
	return action_chain


## Public accessor for action serialisation.


func serialize_actions(actions: Array[Action]) -> Array[Dictionary]:
	return _extract_actions(actions)


## Public accessor for goal serialisation.


func serialize_goals(goals: Array[Goal], agent: GdPAIAgent) -> Array[Dictionary]:
	return _extract_goals(goals, agent)


## Serialises each [Action] in [param actions] into the dictionary format
## expected by the Rust layer, embedding [Callable] references for cost and
## effect evaluation.


func _extract_actions(actions: Array[Action]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for action in actions:
		(
			extracted
			. append(
				{
					"name": action.get_title(),
					"cost_callable": Callable(action, "get_action_cost"),
					"effect_callable": Callable(action, "simulate_effect"),
					"preconditions": _extract_preconditions(action.get_preconditions()),
					"validity_checks": _extract_preconditions(action.get_validity_checks()),
					"requirements": _extract_requirements(action.get_requirements()),
					"provisions": _extract_provisions(action.get_provisions()),
				}
			)
		)
	return extracted


## Serialises each [Precondition] in [param preconditions] into the dictionary
## format expected by the Rust layer by calling [method Precondition.to_bridge_dict]
## on each precondition.


func _extract_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for precond in preconditions:
		extracted.append(precond.to_bridge_dict())
	return extracted


## Serialises each [RequirementSpec] in [param requirements] into the dictionary
## format expected by the Rust layer by calling [method RequirementSpec.to_bridge_dict]
## on each requirement.


func _extract_requirements(requirements: Array[RequirementSpec]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for requirement in requirements:
		extracted.append(requirement.to_bridge_dict())
	return extracted


## Serialises each [ProvisionSpec] in [param provisions] into the dictionary
## format expected by the Rust layer by calling [method ProvisionSpec.to_bridge_dict]
## on each provision.


func _extract_provisions(provisions: Array[ProvisionSpec]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for provision in provisions:
		extracted.append(provision.to_bridge_dict())
	return extracted


## Serialises each [Goal] in [param goals] into the dictionary format expected
## by the Rust layer, evaluating reward and desired state against [param agent].


func _extract_goals(goals: Array[Goal], agent: GdPAIAgent) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for goal in goals:
		(
			extracted
			. append(
				{
					"name": goal.get_title(),
					"reward": goal.compute_reward(agent),
					"desired_state": _extract_preconditions(goal.get_desired_state(agent)),
				}
			)
		)
	return extracted
