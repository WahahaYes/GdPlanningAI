extends GutTest
## Campfire 3D scene integrity: reworked prefabs (skinned agent, KayKit fire
## and wood, reskinned potato) seat correctly in the live scene.

const CAMPFIRE_3D_SCENE: PackedScene = preload("res://examples/campfire_3d.tscn")


func after_each() -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	if scheduler != null:
		scheduler.cancel_all_jobs()
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


func _scheduler() -> GdPAIPlanScheduler:
	return GdPAIAutoload.get_scheduler()


func _pump_frames(frames: int) -> void:
	for i in range(frames):
		await get_tree().physics_frame
		await get_tree().process_frame


func _make_scene() -> Node3D:
	var scene: Node3D = CAMPFIRE_3D_SCENE.instantiate() as Node3D
	for agent_node in scene.find_children("*", "GdPAIAgent", true, false):
		var scene_agent: GdPAIAgent = agent_node as GdPAIAgent
		scene_agent.config.planning_strategy = GdPAIAgentConfig.PlanningStrategy.ON_DEMAND
	add_child_autofree(scene)
	return scene


func test_agent_rests_on_scene_ground() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(30)
	var agent_entity: CharacterBody3D = scene.get_node("Agent1") as CharacterBody3D
	assert_almost_eq(agent_entity.position.y, 0.25, 0.05)


func test_agent_wears_skin_and_socket() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(5)
	assert_not_null(
		scene.get_node("Agent1/Skin/Body").find_child("Knight_Body", true, false),
		"Scene agent should wear the Knight skin"
	)
	assert_not_null(scene.get_node("Agent1/CarrySocket"), "Scene agent should carry a socket")


func test_campfire_has_fire_particles() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(5)
	assert_true(
		scene.get_node("Terrain/Campfire/Fire/FireParticles") is GPUParticles3D,
		"Scene campfire should burn particles, not a CSG glow"
	)


func test_wood_piles_use_log_stacks() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(5)
	for pile_name in ["WoodPile1", "WoodPile2", "WoodPile3", "WoodPile4"]:
		assert_not_null(
			scene.get_node("Terrain/" + pile_name + "/LogStack"),
			"%s should instance the KayKit log stack" % pile_name
		)


func test_spawner_populates_potatoes() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(10)
	var potatoes: Array[Node] = scene.find_children("*", "PotatoObject", true, false)
	assert_eq(potatoes.size(), 5, "Spawner should keep five potatoes in the scene")
	for potato_node in potatoes:
		var potato_entity: Node3D = (potato_node as Node).get_parent() as Node3D
		assert_almost_eq(potato_entity.position.y, 0.25, 0.05)
