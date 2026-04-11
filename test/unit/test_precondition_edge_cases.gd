extends GutTest

# Test to verify precondition behavior with missing properties
func test_missing_property_fails_equal_true():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	# Deliberately NOT setting has_key property
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "RequiresKey",
			"cost_callable": func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
				a.set_property("success", true),
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
			"name": "Succeed",
			"reward": 10.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "success",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_false(result["success"], 
		"Should fail when precondition checks missing property == true")

func test_missing_property_fails_equal_false():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	# Deliberately NOT setting has_key property
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "RequiresNoKey",
			"cost_callable": func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
				a.set_property("success", true),
			"preconditions": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_key",
					"value": false
				}
			],
			"validity_checks": []
		}
	]
	
	var goals: Array[Dictionary] = [
		{
			"name": "Succeed",
			"reward": 10.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "success",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_false(result["success"], 
		"Should fail when precondition checks missing property == false")

func test_has_property_on_missing_property():
	var engine = RustPlanningEngine.new()
	var agent_bb = GdPAIBlackboard.new()
	var world_state = GdPAIBlackboard.new()
	
	var actions: Array[Dictionary] = [
		{
			"name": "RequiresKeyExists",
			"cost_callable": func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
				a.set_property("success", true),
			"preconditions": [
				{
					"target": "agent",
					"operation": "has_property",
					"property_name": "has_key",
					"value": null
				}
			],
			"validity_checks": []
		}
	]
	
	var goals: Array[Dictionary] = [
		{
			"name": "Succeed",
			"reward": 10.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "success",
					"value": true
				}
			]
		}
	]
	
	var result = engine.build_plan(agent_bb, world_state, actions, goals)
	
	assert_false(result["success"], 
		"Should fail when has_property precondition checks missing property")
