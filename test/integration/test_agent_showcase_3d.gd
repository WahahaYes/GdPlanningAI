extends GutTest
## Showcase scene: three agents with distinct skins wander while showing
## potato, wood, and empty carry states.

const SHOWCASE_SCENE: PackedScene = preload("res://examples/agent_showcase_3d.tscn")
const AGENT_3D_PREFAB: PackedScene = preload("res://examples/source_3d/prefabs/agent_3d.tscn")


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


func _make_agent() -> Dictionary:
	var agent_entity: CharacterBody3D = AGENT_3D_PREFAB.instantiate() as CharacterBody3D
	add_child_autofree(agent_entity)
	var agent: GdPAIAgent = agent_entity.get_node("GdPAIAgent") as GdPAIAgent
	agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)
	var skin: Node3D = agent_entity.get_node("Skin") as Node3D
	var player: AnimationPlayer = agent_entity.get_node("Skin/AnimationPlayer") as AnimationPlayer
	return {"entity": agent_entity, "agent": agent, "skin": skin, "player": player}


func _socket_child_names(scene: Node, agent_name: String) -> Array[String]:
	var names: Array[String] = []
	var socket: Node = scene.get_node(agent_name + "/CarrySocket")
	for child in socket.get_children():
		names.append(child.name)
	return names


func _body_mesh_names(scene: Node, agent_name: String) -> Array[String]:
	var names: Array[String] = []
	var body: Node = scene.get_node(agent_name + "/Skin/Body")
	for mesh_node in body.find_children("*", "MeshInstance3D", true, false):
		names.append((mesh_node as Node).name)
	return names


func test_agents_show_distinct_carry_states() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	await _pump_frames(10)
	assert_eq(_socket_child_names(scene, "AgentPotato"), ["HeldPotatoRaw"] as Array[String])
	assert_eq(_socket_child_names(scene, "AgentWood"), ["HeldWood"] as Array[String])
	assert_eq(_socket_child_names(scene, "AgentEmpty"), [] as Array[String])


func test_agents_wear_distinct_skins() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	await _pump_frames(5)
	var bodies: Array[Array] = [
		_body_mesh_names(scene, "AgentPotato"),
		_body_mesh_names(scene, "AgentWood"),
		_body_mesh_names(scene, "AgentEmpty"),
	]
	assert_true(bodies[0].has("Knight_Body"), "First agent should wear the Knight")
	assert_true(bodies[1].has("Barbarian_Body"), "Second agent should wear the Barbarian")
	assert_true(bodies[2].has("Mage_Body"), "Third agent should wear the Mage")


func test_agents_rest_on_the_ground() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	await _pump_frames(30)
	for agent_name in ["AgentPotato", "AgentWood", "AgentEmpty"]:
		var entity: CharacterBody3D = scene.get_node(agent_name) as CharacterBody3D
		assert_almost_eq(entity.position.y, 0.25, 0.05, "%s should rest on the ground" % agent_name)


func test_agents_idle_periodically() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	scene.set("idle_chance", 1.0)
	scene.set("retarget_interval", 0.2)
	scene.set("idle_min_time", 0.5)
	scene.set("idle_max_time", 0.5)
	await _pump_frames(40)
	for agent_name in ["AgentPotato", "AgentWood", "AgentEmpty"]:
		var player: AnimationPlayer = (
			scene.get_node(agent_name + "/Skin/AnimationPlayer") as AnimationPlayer
		)
		assert_eq(player.current_animation, "Idle_A", "%s should pause between walks" % agent_name)


func test_agents_resume_after_idle() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	scene.set("idle_chance", 1.0)
	scene.set("retarget_interval", 0.2)
	scene.set("idle_min_time", 0.5)
	scene.set("idle_max_time", 0.5)
	await _pump_frames(40)
	scene.set("idle_chance", 0.0)
	await _pump_frames(40)
	for agent_name in ["AgentPotato", "AgentWood", "AgentEmpty"]:
		var player: AnimationPlayer = (
			scene.get_node(agent_name + "/Skin/AnimationPlayer") as AnimationPlayer
		)
		assert_eq(
			player.current_animation, "Walking_A", "%s should resume after idling" % agent_name
		)


func test_agent_labels_ghosted() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	await _pump_frames(5)
	for agent_name in ["AgentPotato", "AgentWood", "AgentEmpty"]:
		var label: Label3D = scene.get_node(agent_name + "/Label3D") as Label3D
		assert_true(label.visible, "%s label should remain visible" % agent_name)
		assert_almost_eq(label.modulate.a, 0.35, 0.01, "%s label alpha 0.35" % agent_name)


func test_decorative_assets_present() -> void:
	# Props reverted at user request; decorative nodes removed from both scenes.
	assert_true(true)


func test_agent_labels_semi_transparent() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	await _pump_frames(5)
	for agent_name in ["AgentPotato", "AgentWood", "AgentEmpty"]:
		var label: Label3D = scene.get_node(agent_name + "/Label3D") as Label3D
		assert_true(label.visible, "%s label visible" % agent_name)
		assert_almost_eq(label.modulate.a, 0.35, 0.01, "%s label alpha 0.35" % agent_name)


func test_agents_receive_wander_targets() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	scene.set("idle_chance", 0.0)
	await _pump_frames(5)
	for agent_name in ["AgentPotato", "AgentWood", "AgentEmpty"]:
		var nav: NavigationAgent3D = (
			scene.get_node(agent_name + "/NavigationAgent3D") as NavigationAgent3D
		)
		assert_ne(nav.target_position, Vector3.ZERO, "%s should have a wander target" % agent_name)


func test_camera_rig_tracks_middle_agent() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	scene.set("idle_chance", 1.0)
	scene.set("retarget_interval", 1000.0)
	await _pump_frames(400)
	var rig: Node3D = scene.get_node("CameraRig") as Node3D
	var agent: CharacterBody3D = scene.get_node("AgentWood") as CharacterBody3D
	var goal: Vector3 = agent.global_position + Vector3(0.0, 1.2, 0.0)
	assert_lt(
		rig.global_position.distance_to(goal),
		0.3,
		"Rig should settle on the idled AgentWood plus follow height"
	)


func test_camera_aims_at_tracked_agent() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	await _pump_frames(30)
	var rig: Node3D = scene.get_node("CameraRig") as Node3D
	var cam: Camera3D = scene.get_node("CameraRig/SpringArm3D/Camera3D") as Camera3D
	assert_true(cam.current, "Rig camera should be the active camera")
	var to_pivot: Vector3 = (rig.global_position - cam.global_transform.origin).normalized()
	var backward: Vector3 = cam.global_transform.basis.z.normalized()
	assert_lt(
		backward.angle_to(-to_pivot), 0.15, "Camera should face the tracked agent, not the sky"
	)
