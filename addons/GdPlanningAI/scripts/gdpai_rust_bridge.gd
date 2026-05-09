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
## Returns an ordered [Array[Action]] ready for execution with bindings injected.
## Action-specific bindings from the planner are injected into action instances.
func deserialize_plan_result(result: Dictionary, actions: Array[Action]) -> Array[Action]:
	var action_chain: Array[Action] = []
	
	# Build action_index -> action mapping for binding injection
	var action_index_map: Dictionary = {}
	for i in range(actions.size()):
		action_index_map[i] = actions[i]
	
	# First build the action chain
	for action_index in result.action_chain:
		action_chain.append(actions[action_index])
	
	# Then inject action-specific bindings into action instances
	if result.has("action_bindings"):
		var action_bindings: Array = result.action_bindings
		for binding in action_bindings:
			# Rust sends [action_index, fact_name, [object_ids]]
			var action_idx: int = binding[0]
			var fact_name: String = binding[1]
			var object_ids: Array = binding[2]

			# Inject binding into the action instance
			var action: Action = action_index_map[action_idx]
			if action.has_method("inject_binding"):
				# Convert object_ids to actual object references
				var object_refs = []
				for id in object_ids:
					var obj = instance_from_id(id)
					if obj != null:
						object_refs.append(obj)
				action.inject_binding(fact_name, object_refs)

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
			.append(
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
			.append(
				{
					"name": goal.get_title(),
					"reward": goal.compute_reward(agent),
					"desired_state": _extract_preconditions(goal.get_desired_state(agent)),
				}
			)
		)
	return extracted
