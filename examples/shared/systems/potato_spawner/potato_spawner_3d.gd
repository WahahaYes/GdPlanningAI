class_name PotatoSpawner3D
extends Node

@export var potato_scene: PackedScene
@export var spawn_area_position: Vector3 = Vector3.ZERO
@export var spawn_area_size: Vector3 = Vector3(16, 1, 16)
@export var max_potatoes: int = 5
@export var respawn_time: float = 10.0

var _active_potatoes: Array[Node] = []
var _respawn_timer: float = 0.0


func _ready() -> void:
	for i in range(max_potatoes):
		_spawn_potato()


func _process(delta: float) -> void:
	_active_potatoes = _active_potatoes.filter(func(p): return is_instance_valid(p))

	_respawn_timer += delta
	if _active_potatoes.size() < max_potatoes and _respawn_timer >= respawn_time:
		_spawn_potato()
		_respawn_timer = 0.0


func _spawn_potato() -> void:
	if not potato_scene:
		return

	var potato: Node = potato_scene.instantiate()
	var random_pos: Vector3 = Vector3(
		randf_range(spawn_area_position.x, spawn_area_position.x + spawn_area_size.x),
		spawn_area_position.y,
		randf_range(spawn_area_position.z, spawn_area_position.z + spawn_area_size.z),
	)

	if potato is Node3D:
		potato.position = random_pos

	get_parent().add_child(potato)
	_active_potatoes.append(potato)
