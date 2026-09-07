extends GutTest
## Large campfire variant: 8 fires in a column, corner piles, 8 potatoes,
## 16 scattered agents, and semi-transparent labels throughout.

const LARGE_SCENE: PackedScene = preload("res://examples/campfire_large_3d.tscn")


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
	var scene: Node3D = LARGE_SCENE.instantiate() as Node3D
	for agent_node in scene.find_children("*", "GdPAIAgent", true, false):
		var scene_agent: GdPAIAgent = agent_node as GdPAIAgent
		scene_agent.config.planning_strategy = GdPAIAgentConfig.PlanningStrategy.ON_DEMAND
	add_child_autofree(scene)
	return scene


func test_counts_match_layout() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(10)
	assert_eq(
		scene.find_children("*", "CampfireObject", true, false).size(),
		8,
		"Eight campfires in a column"
	)
	assert_eq(
		scene.find_children("*", "WoodPileObject", true, false).size(), 4, "Four corner wood piles"
	)
	assert_eq(
		scene.find_children("*", "PotatoObject", true, false).size(),
		8,
		"Spawner should keep eight potatoes"
	)
	assert_eq(
		scene.find_children("*", "GdPAIAgent", true, false).size(), 16, "Sixteen scattered agents"
	)


func test_labels_are_semi_transparent() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(5)
	var labels: Array[Node] = scene.find_children("*", "Label3D", true, false)
	assert_gt(labels.size(), 20, "Every agent, fire, pile, and potato carries a label")
	for label_node in labels:
		var label: Label3D = label_node as Label3D
		assert_almost_eq(label.modulate.a, 0.6, 0.01)


func test_potatoes_avoid_every_campfire() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(10)
	var fires: Array[Node] = scene.find_children("*", "CampfireObject", true, false)
	var potatoes: Array[Node] = scene.find_children("*", "PotatoObject", true, false)
	for potato_node in potatoes:
		var potato_entity: Node3D = (potato_node as Node).get_parent() as Node3D
		var clearance: float = INF
		for fire_node in fires:
			var fire_entity: Node3D = (fire_node as Node).get("entity") as Node3D
			var flat: Vector2 = Vector2(
				potato_entity.position.x - fire_entity.position.x,
				potato_entity.position.z - fire_entity.position.z
			)
			clearance = minf(clearance, flat.length())
		assert_true(clearance >= 5.0, "Potatoes should clear every campfire disc")


func test_agents_vary_skins() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(5)
	var seen: Dictionary = {}
	var agents: Array[Node] = scene.find_children("*", "GdPAIAgent", true, false)
	for agent_node in agents:
		var entity: Node3D = (agent_node as Node).get_parent() as Node3D
		var body: Node = entity.get_node("Skin/Body")
		for mesh_node in body.find_children("*", "MeshInstance3D", true, false):
			seen[(mesh_node as Node).name] = true
	for body_name in ["Knight_Body", "Barbarian_Body", "Mage_Body", "Ranger_Body", "Rogue_Body"]:
		assert_true(seen.has(body_name), "Spawned agents should include %s" % body_name)


func test_camera_follows_first_agent() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(30)
	var rig: Node3D = scene.get_node("CameraRig") as Node3D
	var followed: Node3D = rig.get("target") as Node3D
	assert_not_null(followed, "Rig should target a spawned agent")
	assert_eq(followed.name, "Agent1")
	var goal: Vector3 = followed.global_position + Vector3(0.0, 1.2, 0.0)
	assert_lt(rig.global_position.distance_to(goal), 0.6)
	var cam: Camera3D = scene.get_node("CameraRig/SpringArm3D/Camera3D") as Camera3D
	assert_true(cam.current, "Rig camera should be the active camera")
	var to_pivot: Vector3 = (rig.global_position - cam.global_transform.origin).normalized()
	var backward: Vector3 = cam.global_transform.basis.z.normalized()
	assert_lt(backward.angle_to(-to_pivot), 0.15)


func test_agents_rest_on_large_ground() -> void:
	var scene: Node3D = _make_scene()
	await _pump_frames(30)
	var agents: Array[Node] = scene.find_children("*", "GdPAIAgent", true, false)
	for agent_node in agents:
		var entity: CharacterBody3D = (agent_node as Node).get_parent() as CharacterBody3D
		assert_almost_eq(entity.position.y, 0.25, 0.05)
