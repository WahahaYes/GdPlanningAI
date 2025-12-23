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
## Initial blackboard properties to set when this behavior is applied.
var initial_properties: Dictionary = { }


## Apply this behavior configuration to an agent.
func apply_to_agent(agent: GdPAIAgent) -> void:
	for goal in goals:
		agent.goals.append(goal)

	for action in self_actions:
		agent.self_actions.append(action)

	for property in initial_properties:
		agent.blackboard.set_property(property, initial_properties[property])

	for updater in property_updaters:
		updater.initialize(agent)


## Update all property updaters for this behavior.
func update_properties(agent: GdPAIAgent, delta: float) -> void:
	for updater in property_updaters:
		updater.update_properties(agent, delta)
