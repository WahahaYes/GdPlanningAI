class_name WanderAction
extends GoToAction
## Walks to a random nearby point. Reuses [GoToAction]'s navigation with a temporary target
## created at execution time and freed in [method post_perform_action].[br]
## It provides no [code]at_target[/code], so interactions can never be satisfied by wandering.

## Distance of each wander step.
var wander_distance: float
## Temporary node for the wander target location.
var _wander_target_node: Node
## Temporary location data for the wander target.
var _wander_target_location: GdPAILocationData


func _init(p_wander_distance: float) -> void:
	super()
	wander_distance = p_wander_distance


## Returns a fresh instance so plans never share a temporary target.
func clone_for_plan() -> Action:
	return WanderAction.new(wander_distance)


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	return checks


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 1.0


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	return []


# Override
## Records the step without moving the agent. The direction is only drawn at execution, so a
## simulated position would mislead later costs and position checks.
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("wandered", true)


# Override
## Native form of [method simulate_effect]: marks the step without moving the agent.
func get_effect_spec() -> Array:
	return [
		{"op": "set", "target": "agent", "prop": "wandered", "value": true},
	]


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	var location_data: GdPAILocationData = agent.blackboard.get_node_in_group("GdPAILocationData")

	var random_dir: Vector2 = Vector2.from_angle(deg_to_rad(randf_range(-180, 180)))
	var target_pos: Variant

	if location_data.position is Vector2:
		target_pos = location_data.position + random_dir * wander_distance
	else:
		var random_dir_3d: Vector3 = Vector3(random_dir.x, 0, random_dir.y)
		target_pos = location_data.position + random_dir_3d * wander_distance

	# Drop any previous target left behind without cleanup (a replan can orphan
	# one when post_perform_action never ran for it).
	_discard_wander_target()

	# Create temporary node for the target location
	if target_pos is Vector2:
		_wander_target_node = Node2D.new()
	else:
		_wander_target_node = Node3D.new()

	# Parent under the agent's entity so the target dies with the scene even if
	# cleanup never runs (e.g. the run ends mid-wander). Never the tree root: it
	# outlives the scene, leaking every orphaned target at exit.
	agent.entity.add_child(_wander_target_node)
	if target_pos is Vector2:
		(_wander_target_node as Node2D).global_position = target_pos
	else:
		(_wander_target_node as Node3D).global_position = target_pos

	# Create location data for the temporary node
	_wander_target_location = GdPAILocationData.new()
	if target_pos is Vector2:
		_wander_target_location.location_node_2d = _wander_target_node as Node2D
	else:
		_wander_target_location.location_node_3d = _wander_target_node as Node3D
	_wander_target_node.add_child(_wander_target_location)
	# A tree-resident GdPAILocationData would join world snapshots through its
	# GdPAIObjectData groups; this one is a private navigation target, not a
	# world object.
	_wander_target_location.remove_from_group("GdPAILocationData")
	_wander_target_location.remove_from_group("GdPAIObjectData")
	target_location = _wander_target_location

	# Call parent to handle navigation setup
	return super(agent)


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	# Delegate navigation to parent GoToAction
	return super(agent, delta)


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Cleanup is idempotent: the retire path re-posts finished chains, and a
	# target freed by the first cleanup would fail the validity check in
	# GoToAction and log a spurious FAILURE.
	if _wander_target_node == null or not is_instance_valid(_wander_target_node):
		_wander_target_node = null
		_wander_target_location = null
		return Action.Status.SUCCESS
	# Clean up temporary node and location data. The location rides along as a
	# child of the target, so freeing the target frees both.
	_wander_target_node.queue_free()
	_wander_target_node = null
	_wander_target_location = null

	# Call parent to handle navigation cleanup
	return super(agent)


## Drops the current wander target, if any, without waiting for the tree.
func _discard_wander_target() -> void:
	if _wander_target_node != null and is_instance_valid(_wander_target_node):
		_wander_target_node.queue_free()
	_wander_target_node = null
	_wander_target_location = null


# Override
func get_title() -> String:
	return "Wander"


# Override
func get_description() -> String:
	return "Navigate to a random nearby point."
