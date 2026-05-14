class_name FruitTreeObject
extends GdPAIObjectData
## World object representing a fruit tree that drops bananas when shaken.
##[br]
##[br]
## Demonstrates a [GdPAIObjectData] with external cooldown state: validity checks
## on the provided [ShakeTreeAction] inspect [member is_on_cooldown] to prevent
## agents from targeting a tree that was recently shaken.

## Prefab of the fruit to spawn when the tree is shaken.
@export var fruit_prefab: PackedScene
## Radius around the tree in which fruit can land (units match 2D/3D context).
@export var drop_distance: float = 128.0
## Minimum number of fruit dropped per shake.
@export var drop_min_amount: int = 1
## Maximum number of fruit dropped per shake.
@export var drop_max_amount: int = 3
## Seconds the tree must rest between shakes.
@export var cooldown_window: float = 30.0
## Optional [Label] node to display the remaining cooldown time.
@export var cooldown_display: Label
## Reference to the [GdPAIInteractable] component on this object.
@export var interactable_attribs: GdPAIInteractable
## Reference to the [GdPAILocationData] component on this object.
@export var location_data: GdPAILocationData

## Whether the tree is currently on cooldown and cannot be shaken.
var is_on_cooldown: bool = false
var _cooldown_timer: float = 0.0


func _process(delta: float) -> void:
	if is_on_cooldown:
		_cooldown_timer += delta
		if _cooldown_timer >= cooldown_window:
			is_on_cooldown = false
			_cooldown_timer = 0.0

	if cooldown_display != null:
		cooldown_display.text = (
			"%.1f" % (cooldown_window - _cooldown_timer) if is_on_cooldown else ""
		)


## Spawns a random number of fruit in a radius around the tree and starts the cooldown.
func drop_fruit() -> void:
	is_on_cooldown = true
	var amt: int = randi_range(drop_min_amount, drop_max_amount)
	for i in range(amt):
		var fruit_obj: Node = fruit_prefab.instantiate()
		get_tree().root.add_child(fruit_obj)
		if fruit_obj is Node2D:
			fruit_obj.global_position = (
				location_data.position
				+ Vector2(
					randf_range(-drop_distance, drop_distance),
					randf_range(0.5 * drop_distance, drop_distance),
				)
			)
		elif fruit_obj is Node3D:
			fruit_obj.global_position = (
				location_data.position
				+ Vector3(
					randf_range(-drop_distance, drop_distance),
					1.0,
					randf_range(-drop_distance, drop_distance),
				)
			)


# Override
func get_group_labels() -> Array[String]:
	return ["FruitTreeObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	return [ShakeTreeAction.new(self)]


# Override
func get_sim_properties() -> Dictionary:
	return {}
