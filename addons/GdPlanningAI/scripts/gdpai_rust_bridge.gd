class_name GdPAIRustBridge
extends RefCounted

## Internal bridge between GDScript and the Rust planning engine.[br]
##[br]
## Serialises [Action] and [Goal] objects into the plain-dictionary format consumed by
## [code]GdPAIPlanScheduler.submit_plan[/code]. This is where the declarative hooks
## ([method Action.get_preconditions], [method Action.get_validity_checks],
## [method Action.get_requirements], [method Action.get_provisions], [method Goal.compute_reward],
## [method Goal.get_desired_state]) are read, on the main thread, once per plan request. The dictionaries embed
## [Callable] references to [method Action.get_action_cost] and [method Action.simulate_effect];
## the engine calls them back on the main thread during planning, so game state is never copied
## across the language boundary.[br]
##[br]
## Not intended for direct use: [GdPAIAgent] owns and calls this internally.


## Turns a finished planner [param result] into an executable chain.[br]
##[br]
## Each index in [code]result.action_chain[/code] is resolved against [param actions] (the actions
## submitted with the request) and replaced by the result of [method Action.clone_for_plan].
## Returns a dictionary with [code]action_chain[/code] (the ordered [code]Array[Action][/code]) and
## [code]bindings_by_position[/code] (planner bindings as arrays of
## [code][chain_position, fact_name, values][/code], grouped by chain position). The agent injects
## the bindings itself just before executing each step; this method does not.
func deserialize_plan_result(result: Dictionary, actions: Array[Action]) -> Dictionary:
	var action_chain: Array[Action] = []

	var bindings_by_position: Dictionary = {}
	if result.has("action_bindings"):
		var action_bindings: Array = result.action_bindings
		for binding in action_bindings:
			var chain_position: int = binding[0]
			if not bindings_by_position.has(chain_position):
				bindings_by_position[chain_position] = []
			bindings_by_position[chain_position].append(binding)

	for chain_position in range(result.action_chain.size()):
		var action_index: int = result.action_chain[chain_position]
		var action: Action = actions[action_index]
		action_chain.append(action.clone_for_plan())

	return {
		"action_chain": action_chain,
		"bindings_by_position": bindings_by_position,
	}


## Serialises [param actions] for the planner. See [method Action.get_title] and the
## [code]get_*[/code] hooks of [Action] for what is read. Main thread only.
func serialize_actions(actions: Array[Action]) -> Array[Dictionary]:
	return _extract_actions(actions)


## Serialises [param goals] for the planner, evaluating each goal's reward and desired state
## against [param agent] now. Main thread only.
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
