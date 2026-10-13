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
