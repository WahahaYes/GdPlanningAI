extends "res://test/shared/scene_smoke_base.gd"
## Campfire 2D example loads and settles without errors.

const CAMPFIRE_2D_SCENE: PackedScene = preload("res://examples/campfire_2d.tscn")


## The scene instantiates with its world, agent, and campfire present.
func test_campfire_2d_loads_and_settles() -> void:
	var scene: Node2D = CAMPFIRE_2D_SCENE.instantiate() as Node2D
	await _settle_example(scene)
	assert_not_null(scene.get_node_or_null("GdPAIWorldNode"), "Scene should keep its world node")
	assert_gt(
		scene.find_children("*", "GdPAIAgent", true, false).size(), 0, "Scene should keep an agent"
	)
	assert_gt(
		scene.find_children("*", "CampfireObject", true, false).size(),
		0,
		"Scene should keep its campfire"
	)


## A hungry agent with a fueled fire plans something, titles unpinned.
func test_hungry_agent_plans_something() -> void:
	var scene: Node2D = CAMPFIRE_2D_SCENE.instantiate() as Node2D
	await _settle_example(scene)
	var campfire: CampfireObject = GdPAIUTILS.get_child_of_type(
		scene.get_node("Terrain/Campfire"), CampfireObject
	)
	var agent: GdPAIAgent = GdPAIUTILS.get_child_of_type(scene.get_node("Agent1"), GdPAIAgent)
	campfire.current_fuel = 80.0
	agent.blackboard.set_property("hunger", 70.0)
	agent.blackboard.set_property("held_item", "")
	await _pump_frames(3)
	var plan: Array[Action] = await _start_plan_and_wait(agent)
	assert_gt(plan.size(), 0, "Hungry agent with a fueled fire should plan something")


## Starts a plan and waits for the scheduler to return one.
func _start_plan_and_wait(agent: GdPAIAgent, timeout_frames: int = 600) -> Array[Action]:
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
	fail_test("Timed out waiting for agent plan")
	return []
