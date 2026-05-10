class_name PotatoSpawner2D
extends Node

@export var potato_scene: PackedScene
@export var spawn_area: Rect2
@export var max_potatoes: int = 5
@export var respawn_time: float = 10.0

var _active_potatoes: Array[Node] = []
var _respawn_timer: float = 0.0


func _ready() -> void:
	for i in range(max_potatoes):
		_spawn_potato_deferred()


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
	var random_pos: Vector2 = Vector2(
		randf_range(spawn_area.position.x, spawn_area.position.x + spawn_area.size.x),
		randf_range(spawn_area.position.y, spawn_area.position.y + spawn_area.size.y),
	)

	if potato is Node2D:
		potato.position = random_pos

	get_parent().add_child(potato)
	_active_potatoes.append(potato)


func _spawn_potato_deferred() -> void:
	if not potato_scene:
		return

	var potato: Node = potato_scene.instantiate()
	var random_pos: Vector2 = Vector2(
		randf_range(spawn_area.position.x, spawn_area.position.x + spawn_area.size.x),
		randf_range(spawn_area.position.y, spawn_area.position.y + spawn_area.size.y),
	)

	if potato is Node2D:
		potato.position = random_pos

	get_parent().call_deferred("add_child", potato)
	_active_potatoes.append(potato)
