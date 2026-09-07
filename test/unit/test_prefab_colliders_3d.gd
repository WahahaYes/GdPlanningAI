extends GutTest
## Prefab collision shapes rest on the ground instead of sinking into it.

const CAMPFIRE_PREFAB: PackedScene = preload("res://examples/source_3d/prefabs/campfire_3d.tscn")
const POTATO_PREFAB: PackedScene = preload("res://examples/source_3d/prefabs/potato_3d.tscn")
const WOOD_PILE_PREFAB: PackedScene = preload("res://examples/source_3d/prefabs/wood_pile_3d.tscn")
const AGENT_PREFAB: PackedScene = preload("res://examples/source_3d/prefabs/agent_3d.tscn")


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


func _collision_bottom(prefab: PackedScene) -> float:
	var inst: Node3D = prefab.instantiate() as Node3D
	add_child_autofree(inst)
	var agent: GdPAIAgent = inst.get_node_or_null("GdPAIAgent") as GdPAIAgent
	if agent != null:
		agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)
	var bottom: float = INF
	for col in inst.find_children("*", "CollisionShape3D", true, false):
		var shape_node: CollisionShape3D = col as CollisionShape3D
		var local: AABB = _shape_aabb(shape_node.shape)
		var world: AABB = shape_node.transform * local
		bottom = minf(bottom, world.position.y)
	return bottom


func _shape_aabb(shape: Shape3D) -> AABB:
	if shape is BoxShape3D:
		var extents: Vector3 = (shape as BoxShape3D).size * 0.5
		return AABB(-extents, extents * 2.0)
	if shape is SphereShape3D:
		var radius: float = (shape as SphereShape3D).radius
		return AABB(Vector3(-radius, -radius, -radius), Vector3.ONE * radius * 2.0)
	if shape is CylinderShape3D:
		var cylinder: CylinderShape3D = shape as CylinderShape3D
		return AABB(
			Vector3(-cylinder.radius, -cylinder.height * 0.5, -cylinder.radius),
			Vector3(cylinder.radius * 2.0, cylinder.height, cylinder.radius * 2.0)
		)
	return AABB()


func test_campfire_collision_rests_on_ground() -> void:
	assert_almost_eq(_collision_bottom(CAMPFIRE_PREFAB), 0.0, 0.1)


func test_potato_collision_rests_on_ground() -> void:
	assert_almost_eq(_collision_bottom(POTATO_PREFAB), 0.0, 0.1)


func test_wood_pile_collision_rests_on_ground() -> void:
	assert_almost_eq(_collision_bottom(WOOD_PILE_PREFAB), 0.0, 0.1)


func test_agent_collision_rests_on_ground() -> void:
	assert_almost_eq(_collision_bottom(AGENT_PREFAB), 0.0, 0.1)
