class_name Goal
extends RefCounted
## Something an agent wants to achieve. Extend it to write a new goal.[br]
##[br]
## A goal has two jobs: [method compute_reward] says how much the agent wants it right now, and
## [method get_desired_state] says what the world must look like for it to count as achieved. Each
## time the agent submits a plan request, both are read once on the main thread, before the search
## starts, and the planner then looks for the cheapest chain of [Action]s that reaches the
## desired state of the most rewarding goal it can solve. Goals that are already satisfied by the
## initial state are skipped; if every goal is already satisfied, the plan is empty and succeeds.[br]
##[br]
## Goals are registered on an agent through a [GdPAIBehaviorConfig]. Neither hook is called during
## the search or during execution, so both may read live scene state, but they must not use
## [code]await[/code].
## [codeblock]
## class_name StayFedGoal
## extends Goal
##
## func compute_reward(agent: GdPAIAgent) -> float:
##     return agent.blackboard.get_property("hunger")
##
## func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
##     return [Precondition.agent_property_less_than("hunger", 15)]
##
## func get_title() -> String:
##     return "Stay Fed"
## [/codeblock]
## See also [Action] and [Precondition].


# Override
## Returns how rewarding it is to achieve this goal right now.[br]
##[br]
## Called once per plan request on the main thread, before the search. Goals are tried from the
## highest reward to the lowest; the value can depend on dynamic state, for example eating is
## worth more the hungrier the agent is. Return a [float]; [code]0[/code] makes the goal the least
## attractive but does not disable it.
func compute_reward(_agent: GdPAIAgent) -> float:
	return 0


# Override
## Returns the conditions that must hold for this goal to be satisfied.[br]
##[br]
## Called once per plan request on the main thread, before the search, with the owning
## [param _agent]. All returned preconditions must be true at the same time. They are evaluated
## against simulated state, so prefer built-in preconditions such as
## [method Precondition.agent_property_less_than] or
## [method Precondition.world_object_property_geq_than]; they let the planner discover which
## actions satisfy the goal. Return an empty array only for a goal that is always satisfied.
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
	return []


## Returns a short, human-readable name for this goal, shown in logs and debug output. The planner
## sends it to the engine as the goal's name.
func get_title() -> String:
	return "Goal"


## Returns a longer, human-readable description of this goal, for tooling and debug output.
func get_description() -> String:
	return "Base class for Goal."
