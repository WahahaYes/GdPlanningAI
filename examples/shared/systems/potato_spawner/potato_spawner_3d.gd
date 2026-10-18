class_name PotatoSpawner3D
extends Node
## 3D [PotatoSpawner2D] that avoids campfires: their carved navmesh would make a potato inside
## one unreachable. Each spawn takes the first candidate at least
## [member campfire_exclusion_radius] from every fire, or the best of
## [member exclusion_attempts]. Wood piles are not excluded.

## Potato scene instantiated by this spawner.
@export var potato_scene: PackedScene
## Minimum corner of the spawn volume.
@export var spawn_area_position: Vector3 = Vector3.ZERO
## Size of the spawn volume.
@export var spawn_area_size: Vector3 = Vector3(16, 1, 16)
## Maximum number of active potatoes to keep in the scene.
@export var max_potatoes: int = 5
## Seconds to wait before replacing missing potatoes.
@export var respawn_time: float = 10.0
## Radius in meters around each campfire where potatoes never spawn.
@export var campfire_exclusion_radius: float = 5.0
## Placement attempts per spawn before accepting the farthest candidate.
@export var exclusion_attempts: int = 16

var _active_potatoes: Array[Node] = []
var _respawn_timer: float = 0.0
## Campfire XZ positions gathered once at ready.
var _campfire_positions: Array[Vector3] = []


## Collects campfire positions and spawns the starting set of potatoes.
func _ready() -> void:
	_gather_campfire_positions()
	for i in range(max_potatoes):
		_spawn_potato_deferred()


## Prunes freed potatoes and replaces one when the respawn timer allows. The timer runs only
## while a potato is missing, so a dug potato always takes [member respawn_time] to return.
func _process(delta: float) -> void:
	_active_potatoes = _active_potatoes.filter(func(p): return is_instance_valid(p))
	if _active_potatoes.size() >= max_potatoes:
		_respawn_timer = 0.0
		return

	_respawn_timer += delta
	if _respawn_timer >= respawn_time:
		_spawn_potato()
		_respawn_timer = 0.0


## Adds one potato at a clear point in the area.
func _spawn_potato() -> void:
	if not potato_scene:
		return

	var potato: Node = potato_scene.instantiate()
	var random_pos: Vector3 = _random_spawn_position()

	if potato is Node3D:
		potato.position = random_pos

	get_parent().add_child(potato)
	_active_potatoes.append(potato)


## Like [method _spawn_potato] but safe to call while the parent is still in setup.
func _spawn_potato_deferred() -> void:
	if not potato_scene:
		return

	var potato: Node = potato_scene.instantiate()
	var random_pos: Vector3 = _random_spawn_position()

	if potato is Node3D:
		potato.position = random_pos

	get_parent().call_deferred("add_child", potato)
	_active_potatoes.append(potato)


## Collects every campfire position in the scene for spawn exclusion.
func _gather_campfire_positions() -> void:
	_campfire_positions.clear()
	var top: Node = self
	while top.get_parent() != null and not (top.get_parent() is Window):
		top = top.get_parent()
	for node in top.find_children("*", "CampfireObject", true, false):
		var campfire_node: Node = node as Node
		var entity: Node3D = campfire_node.get("entity") as Node3D
		if entity == null:
			entity = campfire_node.get_parent() as Node3D
		if entity != null:
			_campfire_positions.append(entity.global_position)


## Picks a spawn point outside every campfire disc, best effort.
func _random_spawn_position() -> Vector3:
	var best: Vector3 = _candidate_point()
	var best_clearance: float = _clearance(best)
	for i in range(1, exclusion_attempts):
		var candidate: Vector3 = _candidate_point()
		var clearance: float = _clearance(candidate)
		if clearance >= campfire_exclusion_radius:
			return candidate
		if clearance > best_clearance:
			best = candidate
			best_clearance = clearance
	return best


## Draws one uniform point from the spawn volume.
func _candidate_point() -> Vector3:
	return Vector3(
		randf_range(spawn_area_position.x, spawn_area_position.x + spawn_area_size.x),
		spawn_area_position.y,
		randf_range(spawn_area_position.z, spawn_area_position.z + spawn_area_size.z),
	)


## Shortest XZ distance from [param point] to any campfire.
func _clearance(point: Vector3) -> float:
	var clearance: float = INF
	for campfire_pos in _campfire_positions:
		var flat: Vector2 = Vector2(point.x - campfire_pos.x, point.z - campfire_pos.z)
		clearance = minf(clearance, flat.length())
	return clearance
