class_name GdPAIRustBridge
extends RefCounted

## Internal bridge between GDScript and the Rust planning engine.
##
## Serialises [Action] and [Goal] objects into plain dictionaries containing
## [Callable] references, then forwards them to [RustPlanningEngine] for
## depth-first plan search. The Rust layer calls back into GDScript via those
## callables to evaluate costs, effects, and preconditions without copying
## game state across the language boundary.
##
## Not intended for direct use — [GdPAIAgent] owns and calls this internally.

## The Rust planning engine instance.
var planning_engine: RustPlanningEngine

# Override.
func _init() -> void:
	planning_engine = RustPlanningEngine.new()


## Runs the planner and returns the raw result dictionary from Rust.
##
## [param agent] supplies the agent blackboard and world state reference.[br]
## [param actions] is the full set of candidate [Action] objects.[br]
## [param goals] is the full set of candidate [Goal] objects.[br]
## Returns a [Dictionary] with keys [code]success[/code], [code]action_chain[/code],
## [code]total_cost[/code], and [code]goal_index[/code].
func build_plan(
	agent: GdPAIAgent,
	actions: Array[Action],
	goals: Array[Goal]
) -> Dictionary:
	return planning_engine.build_plan(
		agent.blackboard,
		agent.world_node.get_world_state(),
		_extract_actions(actions),
		_extract_goals(goals, agent),
	)


## Resolves the [code]action_chain[/code] indices in [param result] back to
## the original [Action] objects from [param actions].
##
## Returns an ordered [Array[Action]] ready for execution.
func deserialize_plan_result(result: Dictionary, actions: Array[Action]) -> Array[Action]:
	var action_chain: Array[Action] = []
	for action_index in result.action_chain:
		action_chain.append(actions[action_index])
	return action_chain


## Serialises each [Action] in [param actions] into the dictionary format
## expected by the Rust layer, embedding [Callable] references for cost and
## effect evaluation.
func _extract_actions(actions: Array[Action]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for action in actions:
		extracted.append({
			"name": action.get_title(),
			"cost_callable": Callable(action, "get_action_cost"),
			"effect_callable": Callable(action, "simulate_effect"),
			"preconditions": _extract_preconditions(action.get_preconditions()),
			"validity_checks": _extract_preconditions(action.get_validity_checks()),
		})
	return extracted


## Serialises each [Precondition] in [param preconditions] into the dictionary
## format expected by the Rust layer.
##
## [PreconditionBuiltin] entries are encoded as property + operation strings.
## [PreconditionCustom] entries embed a [code]eval_callable[/code] for direct
## invocation from Rust.
func _extract_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for precond in preconditions:
		if precond is PreconditionBuiltin:
			extracted.append({
				"target": _target_to_string(precond.target),
				"operation": _operation_to_string(precond.operation),
				"property_name": precond.property,
				"value": precond.value,
			})
		else:
			extracted.append({
				"operation": "custom_callback",
				"eval_callable": Callable(precond, "evaluate"),
			})
	return extracted


## Converts a [PreconditionBuiltin.Target] enum value to the lowercase string
## token recognised by the Rust [code]parse_operation[/code] parser.
func _target_to_string(target: PreconditionBuiltin.Target) -> String:
	match target:
		PreconditionBuiltin.Target.AGENT:
			return "agent"
		PreconditionBuiltin.Target.WORLD_STATE:
			return "world_state"
		_:
			return "agent"


## Converts a [PreconditionBuiltin.Op] enum value to the lowercase string
## token recognised by the Rust [code]parse_operation[/code] parser.
func _operation_to_string(operation: PreconditionBuiltin.Op) -> String:
	match operation:
		PreconditionBuiltin.Op.HAS_PROPERTY:
			return "has_property"
		PreconditionBuiltin.Op.EQUAL:
			return "equal"
		PreconditionBuiltin.Op.NOT_EQUAL:
			return "not_equal"
		PreconditionBuiltin.Op.GT:
			return "greater_than"
		PreconditionBuiltin.Op.GTE:
			return "greater_than_or_equal"
		PreconditionBuiltin.Op.LT:
			return "less_than"
		PreconditionBuiltin.Op.LTE:
			return "less_than_or_equal"
		_:
			return "has_property"


## Serialises each [Goal] in [param goals] into the dictionary format expected
## by the Rust layer, evaluating reward and desired state against [param agent].
func _extract_goals(goals: Array[Goal], agent: GdPAIAgent) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for goal in goals:
		extracted.append({
			"name": goal.get_title(),
			"reward": goal.compute_reward(agent),
			"desired_state": _extract_preconditions(goal.get_desired_state(agent)),
		})
	return extracted
