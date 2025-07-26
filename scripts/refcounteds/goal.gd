## A GdPAI agent's goal defines a reward level and a required state to satisfy the goal.
class_name Goal
extends RefCounted

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")


## How rewarding is this goal to complete?  This can depend on dynamic factors (i.e. eating is
## more rewarding if an agent is hungry).
func compute_reward(agent: GdPAIAgent) -> float:
	return 0


## What do the agent's blackboard and world state need to contain for the goal to be satisfied?
##[br]
##[br]
## Returns an array of preconditions.
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]:
	return []
