class_name PropertyUpdater
extends RefCounted
## Changes an agent's blackboard properties over time, outside of planning. Extend it for things
## that drift on their own, such as hunger rising or stamina regenerating.[br]
##[br]
## Updaters are registered by a [GdPAIBehaviorConfig] in [method GdPAIBehaviorConfig._populate].
## When the config is applied, [method initialize] is called once; after that
## [method update_properties] is called on the main thread every frame, before the agent plans or
## executes, for as long as the agent exists. Because they run every frame they may use the
## scene tree, but should stay cheap. They are never called during planning, so the planner only
## sees their results through the agent blackboard.
## [codeblock]
## class_name HungerUpdater
## extends PropertyUpdater
##
## func initialize(agent: GdPAIAgent) -> void:
##     agent.blackboard.set_property("hunger", 0.0)
##
## func update_properties(agent: GdPAIAgent, delta: float) -> void:
##     agent.blackboard.set_property("hunger", agent.blackboard.get_property("hunger") + delta)
## [/codeblock]
## Property names must not clash with those written by other behaviors on the same agent.


# Override
## Advances the properties this updater owns. Called on the main thread every frame (from the
## agent's [code]_process[/code]) with the frame time [param _delta] in seconds. Read and write
## through [member GdPAIAgent.blackboard] of [param _agent].
func update_properties(
	_agent: GdPAIAgent,
	_delta: float,
) -> void:
	pass


# Override
## Sets up initial property values. Called once on the main thread when the owning
## [GdPAIBehaviorConfig] is applied to [param _agent] (in the agent's [code]_ready[/code], after
## its blackboard has been created from its [GdPAIBlackboardPlan]), before the first
## [method update_properties].
func initialize(_agent: GdPAIAgent) -> void:
	pass
