class_name PotatoSpawner2D
extends Node
## Keeps up to [member max_potatoes] potatoes in [member spawn_area], replacing dug ones every
## [member respawn_time] seconds. Initial potatoes are added deferred, since the parent is
## still setting up during [method _ready].

## Potato scene instantiated by this spawner.
@export var potato_scene: PackedScene
## Area in which potatoes may spawn.
@export var spawn_area: Rect2
## Maximum number of active potatoes to keep in the scene.
@export var max_potatoes: int = 5
## Seconds to wait before replacing missing potatoes.
@export var respawn_time: float = 10.0

var _active_potatoes: Array[Node] = []
var _respawn_timer: float = 0.0


## Spawns the starting set of potatoes.
func _ready() -> void:
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


## Adds one potato at a random point in the area.
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


## Like [method _spawn_potato] but safe to call while the parent is still in setup.
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
