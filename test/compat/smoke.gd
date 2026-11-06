extends Node
## Smoke test for the oldest supported Godot, run by [code]scripts/compat-smoke.sh[/code] in a
## throwaway project that holds the addon as the release zip lays it out. One agent must plan a
## two-step chain (one native effect spec, one GDScript effect) and run it to the goal. Prints
## [code]COMPAT-SMOKE-OK[/code] and exits 0, or prints [code]COMPAT-SMOKE-FAILED: <why>[/code] and
## exits 1. Built in code, with no scenes, so it never depends on a scene format.

## Frames to wait for the plan to be delivered and run.
const TIMEOUT_FRAMES: int = 600

var _agent: GdPAIAgent
var _delivered: Dictionary = {}
var _frames: int = 0


## Picks up the food: a native effect spec.
class GetFood:
	extends Action

	func get_effect_spec() -> Array:
		return [{"op": "set", "target": "agent", "prop": "has_food", "value": true}]

	func get_action_cost(_agent_bb: GdPAIBlackboard, _world_state: GdPAIBlackboard) -> float:
		return 1.0

	func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
		agent.blackboard.set_property("has_food", true)
		return Action.Status.SUCCESS

	func get_title() -> String:
		return "GetFood"


## Eats the food: a GDScript effect, so a planning callback crosses into GDScript.
class Eat:
	extends Action

	func get_preconditions() -> Array[Precondition]:
		return [Precondition.agent_property_equal_to("has_food", true)]

	func get_action_cost(_agent_bb: GdPAIBlackboard, _world_state: GdPAIBlackboard) -> float:
		return 1.0

	func simulate_effect(agent_bb: GdPAIBlackboard, _world_state: GdPAIBlackboard) -> void:
		agent_bb.set_property("fed", true)

	func perform_action(agent: GdPAIAgent, _delta: float) -> Action.Status:
		agent.blackboard.set_property("fed", true)
		return Action.Status.SUCCESS

	func get_title() -> String:
		return "Eat"


## Wants the agent fed.
class BeFed:
	extends Goal

	func compute_reward(_agent: GdPAIAgent) -> float:
		return 1.0

	func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
		return [Precondition.agent_property_equal_to("fed", true)]

	func get_title() -> String:
		return "BeFed"


## Checks the library loaded, then builds the world and the agent.
func _ready() -> void:
	if not ClassDB.class_exists("GdPAIPlanScheduler"):
		_fail("the GdPlanningAI library did not load (no GdPAIPlanScheduler class)")
		return
	print(
		(
			"COMPAT-SMOKE: Godot %s, library %s"
			% [
				Engine.get_version_info()["string"],
				GdPAIAutoload.get_scheduler().get_engine_version(),
			]
		)
	)

	var world_node: GdPAIWorldNode = GdPAIWorldNode.new()
	world_node.blackboard_plan = GdPAIBlackboardPlan.new()
	add_child(world_node)

	_agent = GdPAIAgent.new()
	_agent.config = GdPAIAgentConfig.new()
	_agent.config.planning_strategy = GdPAIAgentConfig.PlanningStrategy.ON_DEMAND
	_agent.entity = Node.new()
	add_child(_agent.entity)
	add_child(_agent)
	_agent.blackboard.set_property("has_food", false)
	_agent.blackboard.set_property("fed", false)
	_agent.self_actions.append(GetFood.new())
	_agent.self_actions.append(Eat.new())
	_agent.goals.append(BeFed.new())
	_agent.plan_delivered.connect(_on_plan_delivered)
	_agent.manually_start_plan()


## Keeps the delivered plan result.
func _on_plan_delivered(result: Dictionary) -> void:
	_delivered = result


## Waits for the plan, then for the agent to run it.
func _process(_delta: float) -> void:
	if _agent == null:
		return
	_frames += 1
	if not _delivered.is_empty():
		if not _delivered.get("success", false):
			_fail("the planner found no plan: %s" % _delivered)
			return
		if _agent.get_current_plan().size() != 2:
			_fail("expected a two-step plan, got %d steps" % _agent.get_current_plan().size())
			return
		if _agent.blackboard.get_property("fed") == true:
			print("COMPAT-SMOKE-OK")
			get_tree().quit(0)
			return
	if _frames > TIMEOUT_FRAMES:
		_fail(
			"timed out after %d frames (plan delivered: %s)" % [_frames, not _delivered.is_empty()]
		)


## Reports a failure and exits 1.
func _fail(why: String) -> void:
	_agent = null
	printerr("COMPAT-SMOKE-FAILED: %s" % why)
	get_tree().quit(1)
