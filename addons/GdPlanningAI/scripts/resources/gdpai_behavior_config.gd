class_name GdPAIBehaviorConfig
extends Resource
## Base class for agent behavior configurations.
## Extend this class to create specific behavior configurations.
##
## Override [method _populate] to add goals, actions, and property updaters.
## Each call to [method apply_to_agent] builds fresh lists, so sharing this
## resource across multiple agents is safe.


## Apply this behavior configuration to an agent.
## Calls [method _populate] fresh each time to avoid shared-state issues when
## a single resource instance is used by more than one agent.
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


## Override this method to fill [param goals], [param actions], and
## [param updaters] with the behaviours this config provides.
## Called once per agent during [method apply_to_agent].
func _populate(
		_goals: Array[Goal],
		_actions: Array[Action],
		_updaters: Array[PropertyUpdater],
) -> void:
	pass
