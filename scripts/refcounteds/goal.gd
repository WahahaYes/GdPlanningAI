class_name Goal
extends RefCounted
## A GdPAI agent's goal defines a reward level and a required state to satisfy the goal.


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


## Returns a short title for the goal.
func get_title() -> String:
	return "Goal"


## Returns a description of the goal.
func get_description() -> String:
	return "Base class for Goal."
