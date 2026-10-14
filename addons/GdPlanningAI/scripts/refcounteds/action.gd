class_name Action
extends RefCounted
## Base class for everything an agent can do. Extend it to write a new action.[br]
##[br]
## An action has two lives. During [b]planning[/b] the planner never runs the action: it reads the
## action's declarative surfaces ([method get_validity_checks], [method get_preconditions],
## [method get_requirements], [method get_provisions]) and calls [method get_action_cost] and
## [method simulate_effect] on simulated blackboards, to work out whether the action helps reach a
## goal and what it would cost. During [b]execution[/b] the owning [GdPAIAgent] runs the chosen
## chain of actions in real time through [method pre_perform_action], [method perform_action] and
## [method post_perform_action].[br]
##[br]
## [b]Planning-time hooks[/b] ([method get_action_cost], [method simulate_effect] and custom
## precondition callables) are invoked on the main thread by the plan scheduler, once per
## candidate the search considers, while the search itself runs on a background thread. They
## receive simulation snapshots, not live nodes: read and write state through the blackboards and
## [SimObjectProxy] objects, never through the scene tree, and never use [code]await[/code].
## Their results must depend only on their arguments and the action's own fields.[br]
##[br]
## [b]Declarative hooks[/b] ([method get_validity_checks], [method get_preconditions],
## [method get_requirements], [method get_provisions], [method get_title]) are read on the main
## thread each time the agent submits a plan request, so they may inspect the action's fields
## (for example a target node passed to [code]_init[/code]).[br]
##[br]
## [b]Execution hooks[/b] run on the main thread from the agent's [code]_process[/code]. They may
## use the scene tree and any live node. Keep per-run data in [method set_state] rather than in
## member variables, because one action instance can appear several times in a plan.[br]
##[br]
## [b]Optional hook:[/b] if the action defines
## [code]func inject_binding(fact_name: String, value: Array) -> void[/code], the agent calls it
## before each execution hook with the concrete objects the planner bound to one of the action's
## wildcard provisions (see [GoToAction] and [method clone_for_plan]). [Action] itself does not
## declare it; the agent checks for it with [code]has_method[/code].[br]
##[br]
## Typical subclass:
## [codeblock]
## class_name EatAction
## extends Action
##
## var food: GdPAIObjectData
##
## func _init(p_food: GdPAIObjectData) -> void:
##     food = p_food
##
## func get_title() -> String:
##     return "Eat"
##
## func get_validity_checks() -> Array[Precondition]:
##     return [Precondition.check_is_object_valid(food)]
##
## func get_action_cost(_agent_bb: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
##     return 1.0
##
## func simulate_effect(agent_bb: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
##     agent_bb.set_property("hunger", 0.0)
##
## func perform_action(agent: GdPAIAgent, _delta: float) -> Status:
##     agent.blackboard.set_property("hunger", 0.0)
##     return Status.SUCCESS
## [/codeblock]
## See also [Goal], [GoToAction], [Precondition], [RequirementSpec], [ProvisionSpec] and
## [GdPAIObjectData].

## Result of [method pre_perform_action], [method perform_action] and
## [method post_perform_action].
enum Status {
	FAILURE,  ## The action failed. The agent aborts the whole plan and will replan.
	RUNNING,  ## The action is not finished. [method perform_action] is called again next frame.
	SUCCESS,  ## The action finished. The agent moves on to the next action in the chain.
}

## Position of this action instance in the plan being executed, or [code]-1[/code] when it is not
## being executed. Set by the agent before each execution hook and used by [method set_state] to
## keep the data of repeated occurrences of the same action apart. Do not set it yourself.
var chain_position: int = -1


## Returns hard gates that decide whether this action is a planning candidate at all.[br]
##[br]
## Each check is evaluated once per plan request against the [b]initial[/b] state, during
## candidate discovery. An action with any failing check is dropped from that search. Checks do
## not take part in backward chaining and are not re-evaluated against simulated state; use
## [method get_preconditions] for needs the planner should satisfy with other actions.[br]
##[br]
## Prefer built-in checks such as [method Precondition.check_is_object_valid] and
## [method Precondition.agent_has_property]; custom callables make discovery slower.
## Called on the main thread when the plan request is built; a new array is expected each call.
## [codeblock]
## func get_validity_checks() -> Array[Precondition]:
##     return [
##         Precondition.check_is_object_valid(tree_node),
##         Precondition.agent_has_property("entity"),
##     ]
## [/codeblock]
func get_validity_checks() -> Array[Precondition]:
	return []


# Override
## Returns the cost of running this action, used by the planner to prefer cheaper plans.[br]
##[br]
## Called during planning, on the main thread, by the plan scheduler for each candidate the search
## evaluates; the search runs on a background thread and waits for the answer. Both blackboards
## are simulation snapshots, not live state. Reach a simulated object with
## [code]world_state.get_object_for(my_object_data_node)[/code] (a [SimObjectProxy]), or by group
## with [method GdPAIBlackboard.get_proxy_in_group], and read it with [code]get_property[/code].
## Values bound by the planner for this action's provisions (such as [code]at_target[/code] for
## [GoToAction]) are readable on [param _agent_blackboard] as ordinary properties.[br]
##[br]
## Return contract: a non-negative [float]. Return [code]INF[/code] to rule the action out in the
## current context (for example when its target was freed). A non-numeric return value is treated
## as [code]INF[/code] with a warning. Validity checks have already passed when this is called,
## except that live object references may have been freed in the meantime.[br]
##[br]
## [b]Do not use [code]await[/code] and do not access the scene tree from this method.[/b]
## [codeblock]
## func get_action_cost(agent_bb: GdPAIBlackboard, world: GdPAIBlackboard) -> float:
##     var tree_state: SimObjectProxy = world.get_object_for(tree_data)
##     if tree_state == null or tree_state.get_property("fruit") <= 0:
##         return INF
##     return 2.0
## [/codeblock]
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0


## Returns the preconditions the planner must satisfy before this action can run.[br]
##[br]
## The planner chains backward: when a precondition is not true in the current simulated state, it
## searches for an earlier action whose [method simulate_effect] makes it true. Preconditions are
## checked against simulated [GdPAIBlackboard] state, never the live scene tree. Read on the main
## thread when the plan request is built.[br]
##[br]
## Property names must match the names your [method simulate_effect] implementations write.
## [codeblock]
## func get_preconditions() -> Array[Precondition]:
##     return [Precondition.agent_property_greater_than("stamina", 20)]
## [/codeblock]
func get_preconditions() -> Array[Precondition]:
	return []


## Returns symbolic dependencies that must be provided by an earlier action in the chain.[br]
##[br]
## Requirements are matched by name and kind against the [method get_provisions] of other actions,
## without simulation. For example [code]RequirementSpec.fact("at_target", [location])[/code]
## makes the planner place a [GoToAction] (or any action providing that fact) earlier in the plan.
## Read on the main thread when the plan request is built.
## [codeblock]
## func get_requirements() -> Array[RequirementSpec]:
##     return [RequirementSpec.fact("at_target", [location_data])]
## [/codeblock]
func get_requirements() -> Array[RequirementSpec]:
	return []


## Returns the facts and bindings this action offers to later actions in the chain.[br]
##[br]
## They are matched against the [method get_requirements] of other actions. Bindings chosen by the
## planner for a wildcard provision reach the executing action through [method inject_binding].
## Read on the main thread when the plan request is built.
## [codeblock]
## func get_provisions() -> Array[ProvisionSpec]:
##     return [ProvisionSpec.binding("held_item", "wood"), ProvisionSpec.fact("is_food")]
## [/codeblock]
func get_provisions() -> Array[ProvisionSpec]:
	return []


# Override
## Applies this action's effect to the simulated state, so the planner can tell which goals and
## preconditions it satisfies.[br]
##[br]
## Called during planning, on the main thread, by the plan scheduler for each step of a candidate
## plan; the search runs on a background thread. Modify the blackboards in place; there is no
## return value. Reach a simulated object with
## [code]world_state.get_object_for(my_object_data_node)[/code], which returns a [SimObjectProxy];
## writes through its [code]set_property[/code] are reflected in the simulation state. Bindings
## chosen by the planner are readable as properties on [param _agent_blackboard].[br]
##[br]
## Keep it synchronous and deterministic, and keep it consistent with what
## [method perform_action] really does: the planner trusts this model of reality.[br]
##[br]
## [b]Do not use [code]await[/code] and do not access the scene tree from this method.[/b]
## [codeblock]
## func simulate_effect(agent_bb: GdPAIBlackboard, world: GdPAIBlackboard) -> void:
##     agent_bb.set_property("hunger", maxf(0.0, agent_bb.get_property("hunger") - 40.0))
##     var campfire: SimObjectProxy = world.get_object_for(campfire_data)
##     if campfire != null:
##         campfire.set_property("current_fuel", campfire.get_property("current_fuel") + 10)
## [/codeblock]
func simulate_effect(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	pass


# Override
## Prepares the action for execution. Called on the main thread.[br]
##[br]
## When the agent starts executing a freshly planned chain, it calls this on [b]every[/b] action
## in the chain, in order, once, before any [method perform_action] runs (the agent sets
## [member chain_position] and injects bindings first). If an action returns
## [constant Status.FAILURE] the remaining actions' pre-hooks are skipped, the plan is aborted and
## the cleanup phase runs (see [method post_perform_action]).[br]
##[br]
## Use it to validate the live scene (validity checks that held at planning time may be false now),
## cache nodes and initialise per-run data with [method set_state]. Return
## [constant Status.SUCCESS] to proceed. Because every action prepares before the first one runs,
## do not assume earlier actions have already acted.
## [codeblock]
## func pre_perform_action(agent: GdPAIAgent) -> Status:
##     if not is_instance_valid(tree_node):
##         return Status.FAILURE
##     set_state(agent, "elapsed", 0.0)
##     return Status.SUCCESS
## [/codeblock]
func pre_perform_action(_agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


# Override
## Runs the action in the real game. Called on the main thread every frame while this action is
## the current step of the plan.[br]
##[br]
## [param _delta] is the frame time in seconds. Return [constant Status.RUNNING] to be called again
## next frame, [constant Status.SUCCESS] to finish and let the next action start on the following
## frame, or [constant Status.FAILURE] to abort the plan (the agent then replans). Watch for
## conditions that can become false while the action runs, such as a freed target.[br]
##[br]
## [b]Anything that must stop when this action ends has to be stopped here[/b], before returning
## [constant Status.SUCCESS] or [constant Status.FAILURE]: [method post_perform_action] only runs
## when the whole plan ends, not when this single action does.
## [codeblock]
## func perform_action(agent: GdPAIAgent, delta: float) -> Status:
##     if not is_instance_valid(tree_node):
##         return Status.FAILURE
##     var elapsed: float = get_state(agent, "elapsed") + delta
##     set_state(agent, "elapsed", elapsed)
##     return Status.SUCCESS if elapsed >= 2.0 else Status.RUNNING
## [/codeblock]
func perform_action(
	_agent: GdPAIAgent,
	_delta: float,
) -> Status:
	return Status.SUCCESS


# Override
## Cleans up after the plan. Called on the main thread.[br]
##[br]
## This is a guaranteed cleanup phase, but it runs [b]once, for every action in the chain, only
## when the whole plan ends[/b], whether it completed or was aborted by a
## [constant Status.FAILURE]. It does not run when this action alone finishes, and it is not
## called if the plan is replaced by a newer one while still running. Implementations must
## tolerate being called when [method pre_perform_action] or [method perform_action] never ran or
## returned [constant Status.FAILURE] for this action. The return value is ignored.[br]
##[br]
## Use it to erase per-run [method set_state] data and release shared resources.
## [codeblock]
## func post_perform_action(agent: GdPAIAgent) -> Status:
##     erase_state(agent, "elapsed")
##     return Status.SUCCESS
## [/codeblock]
func post_perform_action(_agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


## Stores a per-run value for this action on [param agent]'s blackboard.[br]
##[br]
## The key is prefixed with this instance's id and its [member chain_position], so two occurrences
## of the same action in one plan, or two different actions using the same [param key], do not
## collide. Use it from the execution hooks instead of member variables. Outside execution
## ([member chain_position] is [code]-1[/code]) position [code]0[/code] is used.
func set_state(agent: GdPAIAgent, key: String, value: Variant) -> void:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	agent.blackboard.set_property(str(get_instance_id()) + "_" + pos + "_" + key, value)


## Returns the value stored by [method set_state] for [param key], or [code]null[/code] if it was
## never set or was erased.
func get_state(agent: GdPAIAgent, key: String) -> Variant:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	return agent.blackboard.get_property(str(get_instance_id()) + "_" + pos + "_" + key)


## Removes the value stored by [method set_state] for [param key]. Call it from
## [method post_perform_action] so per-run data does not accumulate on the agent blackboard.
func erase_state(agent: GdPAIAgent, key: String) -> void:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	agent.blackboard.erase_property(str(get_instance_id()) + "_" + pos + "_" + key)


## Returns [code]true[/code] if [method set_state] stored a value for [param key] that has not
## been erased.
func has_state(agent: GdPAIAgent, key: String) -> bool:
	var pos: String = str(chain_position) if chain_position >= 0 else "0"
	return agent.blackboard.has_property(str(get_instance_id()) + "_" + pos + "_" + key)


## Returns the instance that will represent this action in a plan. Called on the main thread
## once per chain step when a plan result is turned into an executable chain.[br]
##[br]
## The default returns [code]self[/code], which is fine when per-run data lives in
## [method set_state]. Override it to return a fresh instance when the action keeps mutable member
## variables, such as bindings filled in by [method inject_binding]; otherwise several
## occurrences of one action in the same plan would share them. [GoToAction] does this.
func clone_for_plan() -> Action:
	return self


## Returns a short, human-readable name for this action, shown in logs and debug output. The
## planner also sends it to the engine as the action's name, so keep it stable.
func get_title() -> String:
	return ""


## Returns a longer, human-readable description of what this action does, for tooling and debug
## output.
func get_description() -> String:
	return "Root action."
