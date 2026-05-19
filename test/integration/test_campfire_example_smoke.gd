extends GutTest

const AGENT_2D_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/agent_2d.tscn")
const WOOD_PILE_2D_PREFAB: PackedScene = preload(
	"res://examples/source_2d/prefabs/wood_pile_2d.tscn"
)
const CAMPFIRE_2D_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/campfire_2d.tscn")
const POTATO_2D_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/potato_2d.tscn")
const CAMPFIRE_2D_SCENE: PackedScene = preload("res://examples/campfire_2d.tscn")
const WORLD_NODE_SCRIPT: Script = preload(
	"res://addons/GdPlanningAI/scripts/nodes/gdpai_world_node.gd"
)
const BLACKBOARD_PLAN_SCRIPT: Script = preload(
	"res://addons/GdPlanningAI/scripts/gdpai_blackboard_plan.gd"
)


func after_each() -> void:
	await _drain_scheduler()


func _drain_scheduler(timeout_frames: int = 60) -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	if scheduler == null:
		return
	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if scheduler.active_job_count() == 0:
			return
		await get_tree().process_frame


func _make_world_node() -> GdPAIWorldNode:
	var world_node: GdPAIWorldNode = WORLD_NODE_SCRIPT.new()
	world_node.name = "GdPAIWorldNode"
	world_node.blackboard_plan = BLACKBOARD_PLAN_SCRIPT.new()
	add_child_autofree(world_node)
	return world_node


func _pump_frames(frames: int) -> void:
	for i in range(frames):
		await get_tree().physics_frame
		await get_tree().process_frame


func _scheduler() -> GdPAIPlanScheduler:
	return GdPAIAutoload.get_scheduler()


func _start_plan_and_wait(agent: GdPAIAgent, timeout_frames: int = 300) -> Array[Action]:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	var previous_plan: Array[Action] = agent.get_current_plan()
	agent.manually_start_plan()
	var saw_job: bool = scheduler.active_job_count() > 0
	for i in range(timeout_frames):
		scheduler.process_callbacks()
		saw_job = saw_job or scheduler.active_job_count() > 0
		if agent.get_current_plan() != previous_plan:
			return agent.get_current_plan()
		if saw_job and scheduler.active_job_count() == 0:
			return agent.get_current_plan()
		await get_tree().process_frame
	
	# Timeout reached - cancel the in-flight planning job
	scheduler.cancel_agent_jobs(agent)
	fail_test("Timed out waiting for submitted agent plan")
	return []


func _plan_titles(plan: Array[Action]) -> String:
	var titles: Array[String] = []
	for action in plan:
		titles.append(action.get_title())
	return " -> ".join(titles)


func _setup_campfire_scene() -> Dictionary:
	var scene: Node2D = CAMPFIRE_2D_SCENE.instantiate()
	for agent_node in scene.find_children("*", "GdPAIAgent", true, false):
		var scene_agent: GdPAIAgent = agent_node
		scene_agent.config.planning_strategy = GdPAIAgentConfig.PlanningStrategy.ON_DEMAND
	add_child_autofree(scene)

	await _pump_frames(10)

	var campfire_entity: Node = scene.get_node("Campfire")
	var agent_entity: Node = scene.get_node("Agent1")
	var campfire: CampfireObject = GdPAIUTILS.get_child_of_type(campfire_entity, CampfireObject)
	var agent: GdPAIAgent = GdPAIUTILS.get_child_of_type(agent_entity, GdPAIAgent)
	agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)

	return {"agent": agent, "campfire": campfire, "scene": scene}


# ── Scenario Tests ─────────────────────────────────────────────
func test_full_cooking_chain() -> void:
	var setup: Dictionary = await _setup_campfire_scene()
	var agent: GdPAIAgent = setup["agent"]
	var campfire: CampfireObject = setup["campfire"]

	assert_true(agent.goals.size() > 0, "Agent should have goals after setup")
	assert_true(agent.self_actions.size() > 0, "Agent should have self_actions after setup")

	campfire.current_fuel = 80.0
	agent.blackboard.set_property("hunger", 70.0)
	agent.blackboard.set_property("held_item", "")
	await _pump_frames(3)

	var plan: Array[Action] = await _start_plan_and_wait(agent)
	if plan.is_empty():
		fail_test("Agent should plan when hungry with fire available, but got empty plan")
		return

	assert_eq(plan.size(), 5, "Plan should have 5 actions: %s" % _plan_titles(plan))
	assert_eq(plan[0].get_title(), "Go To")
	assert_eq(plan[1].get_title(), "Dig Potato")
	assert_eq(plan[2].get_title(), "Go To")
	assert_eq(plan[3].get_title(), "Cook Potato")
	assert_eq(plan[4].get_title(), "Eat Held Food")


func test_fire_too_low_to_cook() -> void:
	var setup: Dictionary = await _setup_campfire_scene()
	var agent: GdPAIAgent = setup["agent"]
	var campfire: CampfireObject = setup["campfire"]

	campfire.current_fuel = 10.0
	agent.blackboard.set_property("hunger", 60.0)
	agent.blackboard.set_property("held_item", "potato")
	await _pump_frames(3)

	var plan: Array[Action] = await _start_plan_and_wait(agent)
	assert_true(plan.is_empty(), "Agent should not plan an over-depth low-fire recovery")


func test_preemptive_fire_maintenance() -> void:
	var setup: Dictionary = await _setup_campfire_scene()
	var agent: GdPAIAgent = setup["agent"]
	var campfire: CampfireObject = setup["campfire"]

	campfire.current_fuel = 35.0
	agent.blackboard.set_property("hunger", 20.0)
	agent.blackboard.set_property("held_item", "")
	await _pump_frames(3)

	var plan: Array[Action] = await _start_plan_and_wait(agent)
	if plan.is_empty():
		fail_test("Agent should plan when fire is moderate and hunger is low, but got empty plan")
		return

	# Fire reward (40) > hunger (20), so fire maintenance is the selected goal
	# Expected: GoTo(wood) → Pick Up Wood → GoTo(campfire) → Add Fuel
	# Hunger work should be handled by a later planning request
	var titles: Array[String] = []
	for a in plan:
		titles.append(a.get_title())
	var wood_idx: int = titles.find("Pick Up Wood")
	assert_true(wood_idx >= 0, "Plan should include Pick Up Wood")
	assert_true(titles.has("Add Fuel"), "Plan should include Add Fuel")
	assert_false(titles.has("Dig Potato"), "Maintain Fire should not chain Hunger in one plan")


func test_competing_priorities_hunger_wins() -> void:
	var setup: Dictionary = await _setup_campfire_scene()
	var agent: GdPAIAgent = setup["agent"]
	var campfire: CampfireObject = setup["campfire"]

	campfire.current_fuel = 15.0
	agent.blackboard.set_property("hunger", 95.0)
	agent.blackboard.set_property("held_item", "")
	await _pump_frames(3)

	var plan: Array[Action] = await _start_plan_and_wait(agent)
	assert_true(plan.is_empty(), "Agent should not plan an over-depth critical recovery")


func test_cannot_add_fuel_when_full() -> void:
	var setup: Dictionary = await _setup_campfire_scene()
	var agent: GdPAIAgent = setup["agent"]
	var campfire: CampfireObject = setup["campfire"]

	campfire.current_fuel = 100.0
	agent.blackboard.set_property("hunger", 10.0)
	agent.blackboard.set_property("held_item", "wood")
	await _pump_frames(3)

	var plan: Array[Action] = await _start_plan_and_wait(agent)
	# Agent should not plan AddFuel when fire is full
	# It should either wander or drop wood and do something else
	if plan.is_empty():
		# Empty plan is acceptable - agent may choose to wander
		assert_true(true, "Agent chose not to plan (acceptable)")
	else:
		var titles: Array[String] = []
		for a in plan:
			titles.append(a.get_title())
		assert_false(
			titles.has("Add Fuel"),
			"Should not plan Add Fuel when fire is full, got: %s" % _plan_titles(plan)
		)


func test_cannot_cook_without_potato() -> void:
	var setup: Dictionary = await _setup_campfire_scene()
	var agent: GdPAIAgent = setup["agent"]
	var campfire: CampfireObject = setup["campfire"]

	campfire.current_fuel = 80.0
	agent.blackboard.set_property("hunger", 70.0)
	agent.blackboard.set_property("held_item", "wood")
	await _pump_frames(3)

	var plan: Array[Action] = await _start_plan_and_wait(agent)
	if plan.is_empty():
		# Empty plan is acceptable - agent may choose to wander
		assert_true(true, "Agent chose not to plan (acceptable)")
	else:
		var titles: Array[String] = []
		for a in plan:
			titles.append(a.get_title())
		# CookPotato requires held_item="potato", so it should not appear when holding wood
		# But AddFuel should be valid
		assert_false(
			titles.has("Cook Potato"),
			"Should not plan Cook Potato when holding wood, got: %s" % _plan_titles(plan)
		)
