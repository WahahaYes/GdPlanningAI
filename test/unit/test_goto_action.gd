extends GutTest
## GoToAction ends a blocked walk by arriving only when the agent is near its path's end.

## Frames to drive the action; covers GoToAction.STUCK_WINDOW at 60 fps with margin.
const DRIVE_FRAMES: int = 90


## A navmesh quad with a stationary (blocked) agent at the origin.
func _make_world() -> Dictionary:
	var region: NavigationRegion3D = NavigationRegion3D.new()
	var mesh: NavigationMesh = NavigationMesh.new()
	mesh.vertices = PackedVector3Array(
		[Vector3(-20, 0, -20), Vector3(20, 0, -20), Vector3(20, 0, 20), Vector3(-20, 0, 20)]
	)
	mesh.add_polygon(PackedInt32Array([0, 1, 2]))
	mesh.add_polygon(PackedInt32Array([0, 2, 3]))
	region.navigation_mesh = mesh
	add_child_autofree(region)

	var body: CharacterBody3D = CharacterBody3D.new()
	var nav_agent: NavigationAgent3D = NavigationAgent3D.new()
	body.add_child(nav_agent)
	var location: GdPAILocationData = GdPAILocationData.new()
	location.location_node_3d = body
	location.entity = body
	body.add_child(location)
	var agent: GdPAIAgent = GdPAIAgent.new()
	agent.entity = body
	body.add_child(agent)
	add_child_autofree(body)
	return {"agent": agent, "nav_agent": nav_agent}


## Returns location data for a target placed at [param position].
func _make_target(position: Vector3) -> GdPAILocationData:
	var node: Node3D = Node3D.new()
	add_child_autofree(node)
	node.global_position = position
	var location: GdPAILocationData = GdPAILocationData.new()
	location.location_node_3d = node
	location.entity = node
	node.add_child(location)
	return location


## Runs [param action] until it stops returning RUNNING; returns its final status.
func _drive(action: GoToAction, agent: GdPAIAgent) -> Action.Status:
	action.chain_position = 0
	assert_eq(action.pre_perform_action(agent), Action.Status.SUCCESS)
	var status: Action.Status = Action.Status.RUNNING
	for i in DRIVE_FRAMES:
		status = action.perform_action(agent, 1.0 / 60.0)
		if status != Action.Status.RUNNING:
			break
		await get_tree().physics_frame
	return status


## Blocked far from the destination: the action fails so the agent replans, and stops walking.
func test_blocked_far_from_target_fails() -> void:
	var world: Dictionary = _make_world()
	await get_tree().physics_frame
	var action: GoToAction = GoToAction.new(_make_target(Vector3(10, 0, 0)))
	var status: Action.Status = await _drive(action, world.agent)
	assert_eq(status, Action.Status.FAILURE, "A blocked agent 10 m out has not arrived")
	assert_almost_eq(
		(world.nav_agent as NavigationAgent3D).target_position,
		(world.agent as GdPAIAgent).entity.global_position,
		Vector3.ONE * 0.01,
		"Navigation should stop where the agent stands"
	)
	assert_false(action.get_state(world.agent, "target_reached"))


## Blocked just short of the destination: as close as it gets, so the action succeeds.
func test_blocked_near_target_arrives() -> void:
	var world: Dictionary = _make_world()
	await get_tree().physics_frame
	var action: GoToAction = GoToAction.new(_make_target(Vector3(1.5, 0, 0)))
	var status: Action.Status = await _drive(action, world.agent)
	assert_eq(status, Action.Status.SUCCESS, "Within the stuck arrival radius counts as arrived")
	assert_true(action.get_state(world.agent, "target_reached"))
