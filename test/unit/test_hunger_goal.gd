extends GutTest
## HungerGoal desired state: satisfied while sated, reachable once hungry.


## Returns an agent whose blackboard holds only the given hunger.
func _agent_with_hunger(hunger: float) -> GdPAIAgent:
	var agent: GdPAIAgent = autofree(GdPAIAgent.new())
	agent.blackboard = GdPAIBlackboard.new()
	agent.blackboard.set_property("hunger", hunger)
	return agent


## Returns the hunger threshold the goal asks the planner to reach.
func _required_hunger(hunger: float) -> float:
	var desired: Array[Precondition] = HungerGoal.new().get_desired_state(
		_agent_with_hunger(hunger)
	)
	assert_eq(desired.size(), 1)
	return (desired[0] as PreconditionBuiltin).value


func test_sated_agent_already_satisfies_goal():
	var hunger: float = 6.9
	assert_lt(hunger, _required_hunger(hunger), "a sated agent must not need to eat")


func test_never_requires_hunger_below_sated():
	assert_eq(_required_hunger(0.0), 15.0)
	assert_eq(_required_hunger(20.0), 15.0)


func test_hungry_agent_aims_one_meal_lower():
	assert_eq(_required_hunger(40.0), 25.0)
