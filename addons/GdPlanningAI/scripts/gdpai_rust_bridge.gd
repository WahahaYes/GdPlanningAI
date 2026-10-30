class_name GdPAIRustBridge
extends RefCounted

## Internal bridge to the Rust engine; [GdPAIAgent] owns it.[br]
## Serialises actions and goals for [code]GdPAIPlanScheduler.submit_plan[/code], reading their
## declarative hooks on the main thread. Cost and effect hooks are passed as [Callable]s, so game
## state never crosses the language boundary.


## Turns a planner [param result] into [code]action_chain[/code] (cloned via
## [method Action.clone_for_plan]) and [code]bindings_by_position[/code] for the agent to inject.
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


## Serialises [param actions] for the planner. Main thread only.
func serialize_actions(actions: Array[Action]) -> Array[Dictionary]:
	return _extract_actions(actions)


## Serialises [param goals], evaluating reward and desired state against [param agent] now.
func serialize_goals(goals: Array[Goal], agent: GdPAIAgent) -> Array[Dictionary]:
	return _extract_goals(goals, agent)


## Serialises each action, embedding [Callable]s for cost and effect.
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
					"effect_spec": action.get_effect_spec(),
					"preconditions": _extract_preconditions(action.get_preconditions()),
					"validity_checks": _extract_preconditions(action.get_validity_checks()),
					"requirements": _extract_requirements(action.get_requirements()),
					"provisions": _extract_provisions(action.get_provisions()),
				}
			)
		)
	return extracted


## Serialises preconditions via [method Precondition.to_bridge_dict].
func _extract_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for precond in preconditions:
		extracted.append(precond.to_bridge_dict())
	return extracted


## Serialises requirements via [method RequirementSpec.to_bridge_dict].
func _extract_requirements(requirements: Array[RequirementSpec]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for requirement in requirements:
		extracted.append(requirement.to_bridge_dict())
	return extracted


## Serialises provisions via [method ProvisionSpec.to_bridge_dict].
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
