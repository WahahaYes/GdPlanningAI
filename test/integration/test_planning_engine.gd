extends GutTest

func test_empty_plan_returns_failure():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = []
	var goals: Array[Dictionary] = []
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_false(result["success"], "Empty plan should return failure")
	assert_eq(result["goal_index"], -1, "Goal index should be -1 on failure")

func test_trivial_goal_already_satisfied():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	agent_bb.set_property("has_key", true)
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = []
	
	var goals: Array[Dictionary] = [
		{
			"name": "AlreadySatisfiedGoal",
			"reward": 10.0,
			"desired_state": []
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_true(result["success"], "Already satisfied goal should succeed")
	assert_eq(result["total_cost"], 0.0, "Already satisfied goal should have zero cost")
	assert_eq(result["action_chain"].size(), 0,
		"Already satisfied goal should have empty action chain")

func test_simple_one_action_plan():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	agent_bb.set_property("has_key", false)
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "GetKey",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_key", true),
			"preconditions": [],
			"validity_checks": []
		}
	]
	
	var goals: Array[Dictionary] = [
		{
			"name": "HaveKey",
			"reward": 10.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_key",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_true(result["success"], "Simple one-action plan should succeed")
	assert_eq(result["action_chain"].size(), 1, "Plan should have 1 action")
	assert_eq(result["action_chain"][0], 0, "Plan should use the first action")
	assert_eq(result["total_cost"], 1.0, "Total cost should be 1.0")

func test_chooses_lower_cost_plan():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	agent_bb.set_property("has_key", false)
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "ExpensiveGetKey",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 10.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_key", true),
			"preconditions": [],
			"validity_checks": []
		},
		{
			"name": "CheapGetKey",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_key", true),
			"preconditions": [],
			"validity_checks": []
		}
	]
	
	var goals: Array[Dictionary] = [
		{
			"name": "HaveKey",
			"reward": 100.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_key",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_true(result["success"], "Plan should succeed")
	assert_eq(result["total_cost"], 1.0, "Should choose cheaper action with cost 1.0")
	assert_eq(result["action_chain"][0], 1, "Should choose CheapGetKey (index 1)")

func test_chooses_cheaper_deeper_chain_over_direct_expensive_completion():
	var engine = RustPlanningEngine.new()
	engine.set_max_recursion(3)
	var agent_bb = GdPAIBlackboard.new()
	agent_bb.set_property("has_food", false)
	agent_bb.set_property("has_fire", false)
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "ExpensiveDirectCampfirePrep",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 10.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_food", true)
				agent.set_property("has_fire", true),
			"preconditions": [],
			"validity_checks": []
		},
		{
			"name": "GetFood",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_food", true),
			"preconditions": [],
			"validity_checks": []
		},
		{
			"name": "LightFire",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_fire", true),
			"preconditions": [],
			"validity_checks": []
		}
	]
	
	var goals: Array[Dictionary] = [
		{
			"name": "ReadyCampfireMeal",
			"reward": 100.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_food",
					"value": true
				},
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_fire",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_true(result["success"], "Plan should succeed")
	assert_eq(result["total_cost"], 2.0, "Should prefer the cheaper two-step chain")
	assert_eq(result["action_chain"], [1, 2], "Should choose GetFood then LightFire")
