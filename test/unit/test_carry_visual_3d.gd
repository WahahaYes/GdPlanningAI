extends GutTest
## Carry-socket visuals follow the agent's held_item blackboard property.

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
	var agent_entity: Node3D = AGENT_3D_PREFAB.instantiate() as Node3D
	add_child_autofree(agent_entity)
	var agent: GdPAIAgent = agent_entity.get_node("GdPAIAgent") as GdPAIAgent
	agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)
	var socket: Node3D = agent_entity.get_node("CarrySocket") as Node3D
	return {"agent": agent, "socket": socket}


func _socket_child_names(socket: Node3D) -> Array[String]:
	var names: Array[String] = []
	for child in socket.get_children():
		names.append(child.name)
	return names


func test_socket_starts_empty() -> void:
	var setup: Dictionary = _make_agent()
	var socket: Node3D = setup["socket"]
	await _pump_frames(5)
	assert_eq(socket.get_child_count(), 0, "Socket should show nothing with empty held_item")


func test_raw_potato_visual_appears() -> void:
	var setup: Dictionary = _make_agent()
	var agent: GdPAIAgent = setup["agent"]
	var socket: Node3D = setup["socket"]
	agent.blackboard.set_property("held_item", "potato")
	await _pump_frames(3)
	assert_eq(_socket_child_names(socket), ["HeldPotatoRaw"] as Array[String])


func test_cooked_potato_visual_swaps() -> void:
	var setup: Dictionary = _make_agent()
	var agent: GdPAIAgent = setup["agent"]
	var socket: Node3D = setup["socket"]
	agent.blackboard.set_property("held_item", "potato")
	await _pump_frames(3)
	agent.blackboard.set_property("held_item", "cooked_potato")
	await _pump_frames(3)
	assert_eq(_socket_child_names(socket), ["HeldPotatoCooked"] as Array[String])


func test_unmapped_item_shows_nothing() -> void:
	var setup: Dictionary = _make_agent()
	var agent: GdPAIAgent = setup["agent"]
	var socket: Node3D = setup["socket"]
	agent.blackboard.set_property("held_item", "wood")
	await _pump_frames(3)
	assert_eq(socket.get_child_count(), 0, "Unmapped held_item ids should show nothing")


func test_clearing_held_item_removes_visual() -> void:
	var setup: Dictionary = _make_agent()
	var agent: GdPAIAgent = setup["agent"]
	var socket: Node3D = setup["socket"]
	agent.blackboard.set_property("held_item", "potato")
	await _pump_frames(3)
	agent.blackboard.set_property("held_item", "")
	await _pump_frames(3)
	assert_eq(socket.get_child_count(), 0, "Socket should clear when held_item empties")
