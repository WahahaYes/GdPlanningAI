class_name GdPAIBehaviorConfig
extends Resource
## A reusable bundle of goals, actions and property updaters (e.g. "hunger", "wander").
## Override [method _populate] and add the resource to [member GdPAIAgentConfig.behavior_configs].
## One resource can be shared by several agents.


## Adds fresh instances from [method _populate] to [param agent] and initializes its updaters.
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


## Override: append new goals, actions and updaters. Called once per agent.
func _populate(
	_goals: Array[Goal],
	_actions: Array[Action],
	_updaters: Array[PropertyUpdater],
) -> void:
	pass
