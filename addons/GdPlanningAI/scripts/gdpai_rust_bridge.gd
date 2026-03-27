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


func _init() -> void:
	planning_engine = RustPlanningEngine.new()


## Runs the planner and returns the raw result dictionary from Rust.
##
## [param agent_blackboard] is the agent's own [GdPAIBlackboard].[br]
## [param world_state] is the current world [GdPAIBlackboard].[br]
## [param actions] is the full set of candidate [Action] objects.[br]
## [param goals] is the full set of candidate [Goal] objects.[br]
## [param agent] is required for goal reward and desired-state evaluation.[br]
## Returns a [Dictionary] with keys [code]success[/code], [code]action_chain[/code],
## [code]total_cost[/code], and [code]goal_index[/code].
func build_plan(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
	actions: Array[Action],
	goals: Array[Goal],
	agent: GdPAIAgent,
) -> Dictionary:
	return planning_engine.build_plan(
		agent_blackboard,
		world_state,
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


## Public accessor for action serialisation — used by the async planning path.
func serialize_actions(actions: Array[Action]) -> Array[Dictionary]:
	return _extract_actions(actions)


## Public accessor for goal serialisation — used by the async planning path.
func serialize_goals(goals: Array[Goal], agent: GdPAIAgent) -> Array[Dictionary]:
	return _extract_goals(goals, agent)


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
## format expected by the Rust layer by calling [method Precondition._to_bridge_dict]
## on each precondition.
func _extract_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for precond in preconditions:
		extracted.append(precond.to_bridge_dict())
	return extracted


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
