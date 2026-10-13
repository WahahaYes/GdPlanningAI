extends GutTest
## MaintainFireGoal tends any campfire by default, or one home group when set.


## Returns the group named by the goal's single desired-state precondition.
func _desired_group(goal: MaintainFireGoal) -> String:
	var desired: Array[Precondition] = goal.get_desired_state(null)
	assert_eq(desired.size(), 1)
	return (desired[0] as PreconditionBuiltin).group


func test_unset_group_tends_any_campfire():
	assert_eq(_desired_group(MaintainFireGoal.new()), "CampfireObject")


func test_home_group_is_tended():
	assert_eq(_desired_group(MaintainFireGoal.new(40.0, 60.0, "HomeCampfire2")), "HomeCampfire2")


func test_each_plan_asks_for_one_step_up():
	var goal: MaintainFireGoal = MaintainFireGoal.new(40.0, 60.0)
	assert_eq(goal.target_fuel(0.0), 1.0, "A dead fire only needs to rise, not reach 60")
	assert_eq(goal.target_fuel(25.9), 26.9, "Below 30 one delivery must still satisfy the goal")
	assert_eq(goal.target_fuel(59.5), 60.0, "Capped at the desired level")
	assert_eq(goal.target_fuel(80.0), 60.0, "Above the desired level the goal is met")


func test_unknown_fuel_falls_back_to_desired_level():
	assert_eq(MaintainFireGoal.new(40.0, 60.0).target_fuel(-1.0), 60.0)
