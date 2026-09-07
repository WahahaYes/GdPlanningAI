extends GutTest
## Showcase scene: three agents with distinct skins wander while showing
## potato, wood, and empty carry states.

const SHOWCASE_SCENE: PackedScene = preload("res://examples/agent_showcase_3d.tscn")


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


func test_agent_labels_hidden() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)
	await _pump_frames(2)
	for agent_name in ["AgentPotato", "AgentWood", "AgentEmpty"]:
		var label: Label3D = scene.get_node(agent_name + "/Label3D") as Label3D
		assert_false(label.visible, "%s debug label should be hidden" % agent_name)


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
	await _pump_frames(30)
	var rig: Node3D = scene.get_node("CameraRig") as Node3D
	var agent: CharacterBody3D = scene.get_node("AgentWood") as CharacterBody3D
	var goal: Vector3 = agent.global_position + Vector3(0.0, 1.2, 0.0)
	assert_lt(
		rig.global_position.distance_to(goal), 0.6, "Rig should track AgentWood plus follow height"
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
