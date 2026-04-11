extends GutTest

func test_engine_instantiation():
	var engine = RustPlanningEngine.new()
	assert_not_null(engine, "RustPlanningEngine should instantiate")

func test_set_max_recursion():
	var engine = RustPlanningEngine.new()
	
	engine.set_max_recursion(5)
	assert_eq(engine.get_max_recursion(), 5, "Should set max recursion to 5")
	
	engine.set_max_recursion(20)
	assert_eq(engine.get_max_recursion(), 20, "Should update max recursion to 20")

func test_empty_blackboards_with_goals():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = []
	var goals: Array[Dictionary] = [
		{
			"name": "TestGoal",
			"reward": 5.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "has_property",
					"property_name": "some_prop",
					"value": null
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_false(result["success"], "Should fail when goal not satisfiable")
	assert_eq(result["goal_index"], -1, "Failed plan has goal_index -1")

func test_multiple_goals_chooses_highest_reward():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = []
	var goals: Array[Dictionary] = [
		{
			"name": "LowRewardGoal",
			"reward": 5.0,
			"desired_state": []
		},
		{
			"name": "HighRewardGoal",
			"reward": 100.0,
			"desired_state": []
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_true(result["success"], "Should succeed with satisfied goals")
	assert_eq(result["goal_index"], 1, "Should choose higher reward goal")

func test_successful_single_action_plan():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	agent_bb.set_property("has_item", false)
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "PickUpItem",
			"cost_callable": func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float:
				return 5.0,
			"effect_callable": func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
				a.set_property("has_item", true),
			"preconditions": [],
			"validity_checks": []
		}
	]
	
	var goals: Array[Dictionary] = [
		{
			"name": "ObtainItem",
			"reward": 50.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_item",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_true(result["success"], "Should successfully find plan")
	assert_eq(result["action_chain"].size(), 1, "Should have 1 action in plan")
	assert_eq(result["action_chain"][0], 0, "Should use PickUpItem action")
	assert_eq(result["total_cost"], 5.0, "Total cost should be 5.0")
	assert_eq(result["goal_index"], 0, "Should satisfy first goal")

func test_action_with_failed_precondition():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	agent_bb.set_property("has_key", false)
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "LockedAction",
			"cost_callable": func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
				a.set_property("goal_met", true),
			"preconditions": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_key",
					"value": true
				}
			],
			"validity_checks": []
		}
	]
	
	var goals: Array[Dictionary] = [
		{
			"name": "Goal",
			"reward": 10.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "goal_met",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_false(result["success"], "Should fail when preconditions not met")
