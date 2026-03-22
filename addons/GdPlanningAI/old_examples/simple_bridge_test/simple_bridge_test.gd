extends Node
## Simple end-to-end test for GdPAIRustBridge.
## Tests the minimal planning path: one action, one goal, no world objects, no spatial logic.
## Scenario: agent has energy=0, GainEnergyAction adds 10, goal requires energy>=10.

var test_passed: bool = false
var test_message: String = ""


## Minimal action: adds energy to the agent blackboard. No world state access.
class GainEnergyAction extends Action:
	func get_action_cost(_agent_bb: GdPAIBlackboard, _world_bb: GdPAIBlackboard) -> float:
		return 5.0

	func simulate_effect(agent_bb: GdPAIBlackboard, _world_bb: GdPAIBlackboard) -> void:
		var current: float = agent_bb.get_property("energy")
		agent_bb.set_property("energy", current + 10.0)

	func get_title() -> String:
		return "Gain Energy"


## Minimal goal: agent energy must reach at least 10.
class HasEnoughEnergyGoal extends Goal:
	func compute_reward(_agent: GdPAIAgent) -> float:
		return 100.0

	func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
		return [Precondition.agent_property_geq_than("energy", 10.0)]

	func get_title() -> String:
		return "Has Enough Energy"


func _ready() -> void:
	await get_tree().process_frame
	_run_test()
	_report_result()


func _run_test() -> void:
	var agent := _setup_agent()
	var world_node := _setup_world(agent)

	var action := GainEnergyAction.new()
	var goal := HasEnoughEnergyGoal.new()

	print("[SimpleTest] Agent blackboard: ", agent.blackboard.get_dict())
	print("[SimpleTest] World state: ", world_node.world_state.get_dict())
	print("[SimpleTest] Calling bridge.build_plan...")

	var bridge := GdPAIRustBridge.new()
	var actions: Array[Action] = [action]
	var goals: Array[Goal] = [goal]
	var result: Dictionary = bridge.build_plan(
		agent.blackboard,
		world_node.world_state,
		actions,
		goals,
		agent,
	)

	print("[SimpleTest] Result: ", result)
	_verify_result(result, action)


func _setup_agent() -> GdPAIAgent:
	var agent := GdPAIAgent.new()
	agent.name = "SimpleTestAgent"
	var config := GdPAIAgentConfig.new()
	config.blackboard_plan = GdPAIBlackboardPlan.new()
	config.blackboard_plan.blackboard_backend = {"energy": 0.0}
	agent.config = config
	agent.entity = agent
	add_child(agent)
	return agent


func _setup_world(agent: GdPAIAgent) -> GdPAIWorldNode:
	var world_node := GdPAIWorldNode.new()
	world_node.name = "SimpleTestWorld"
	var world_plan := GdPAIBlackboardPlan.new()
	world_plan.blackboard_backend = {}
	world_node.blackboard_plan = world_plan
	add_child(world_node)
	agent.world_node = world_node
	return world_node


func _verify_result(result: Dictionary, action: Action) -> void:
	if not result.get("success", false):
		test_message = "Plan failed - no solution found. Result: %s" % str(result)
		return

	var chain: Array = result.get("action_chain", [])
	if chain.is_empty():
		test_message = "Plan succeeded but action chain is empty"
		return

	if int(chain[0]) != 0:
		test_message = "Expected action index 0 but got '%s'" % chain[0]
		return

	var cost: float = result.get("total_cost", INF)
	if cost != 5.0:
		test_message = "Expected cost 5.0 but got %f" % cost
		return

	test_passed = true
	test_message = "SUCCESS: 1-action plan '%s', cost %.1f" % [action.get_title(), cost]


func _report_result() -> void:
	if test_passed:
		prints("[TEST PASSED]", test_message)
	else:
		push_error("[TEST FAILED] " + test_message)
	get_tree().quit()
