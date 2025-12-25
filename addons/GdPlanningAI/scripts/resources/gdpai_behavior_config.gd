class_name GdPAIBehaviorConfig
extends Resource
## Base class for agent behavior configurations.
## Extend this class to create specific behavior configurations.

## Goals that this agent should pursue.
var goals: Array[Goal] = []
## Actions that are always available to this agent.
var self_actions: Array[Action] = []
## Property updaters that modify blackboard properties over time.
var property_updaters: Array[PropertyUpdater] = []
## Whether or not this Resource has initialized its own 
## goals, self_actions, and property_updaters.
var _is_initialized: bool = false


## Apply this behavior configuration to an agent.
func apply_to_agent(agent: GdPAIAgent) -> void:
	# Ensure self_init has completed before applying
	if not _is_initialized:
		_self_init()

	for goal in goals:
		agent.goals.append(goal)

	for action in self_actions:
		agent.self_actions.append(action)

	for updater in property_updaters:
		updater.initialize(agent)


## Update all property updaters for this behavior.
func update_properties(agent: GdPAIAgent, delta: float) -> void:
	for updater in property_updaters:
		updater.update_properties(agent, delta)


## Called after @export values are applied.
## Override this method to set up goals, actions, and property updaters.
func _self_init():
	_is_initialized = true
