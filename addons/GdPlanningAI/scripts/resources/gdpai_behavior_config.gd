class_name GdPAIBehaviorConfig
extends Resource
## A reusable bundle of goals, actions and property updaters for an agent (for example
## "hunger" or "wander"). Extend it and override [method _populate]; then add the resource to
## [member GdPAIAgentConfig.behavior_configs].[br]
##[br]
## Each call to [method apply_to_agent] builds fresh instances, so one resource can safely be shared
## by several agents. Property names written by different behaviors on the same agent must not
## clash.
## [codeblock]
## class_name HungerBehaviorConfig
## extends GdPAIBehaviorConfig
##
## func _populate(
##         goals: Array[Goal],
##         actions: Array[Action],
##         updaters: Array[PropertyUpdater],
## ) -> void:
##     goals.append(StayFedGoal.new())
##     actions.append(GoToAction.new())
##     updaters.append(HungerUpdater.new())
## [/codeblock]


## Adds this behavior's goals, actions and updaters to [param agent]. Called by the agent in its
## [code]_ready[/code]; calls [method _populate] fresh each time to avoid shared state, then calls
## [method PropertyUpdater.initialize] on every new updater.
func apply_to_agent(agent: GdPAIAgent) -> void:
	var local_goals: Array[Goal] = []
	var local_actions: Array[Action] = []
	var local_updaters: Array[PropertyUpdater] = []
	_populate(local_goals, local_actions, local_updaters)
	agent.goals.append_array(local_goals)
	agent.self_actions.append_array(local_actions)
	for updater in local_updaters:
		updater.initialize(agent)
	agent.property_updaters.append_array(local_updaters)


## Override to append the behavior's instances to [param _goals], [param _actions] and
## [param _updaters]. Create new objects on every call. Called once per agent during
## [method apply_to_agent], on the main thread, before the agent's first frame.
func _populate(
	_goals: Array[Goal],
	_actions: Array[Action],
	_updaters: Array[PropertyUpdater],
) -> void:
	pass
