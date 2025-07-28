## A GdPAI demo object for a fruit tree.  The fruit tree provides an action to shake the tree,
## which drops some fruit nearby.  NOTE: it's better practice to keep the concrete behavior
## separate from the AI.  A tree dropping fruit shouldn't depend on being part of the AI framework.
## Because this is a simple example, I have it all bundled together.
class_name SampleFruitTreeObject
extends GdPAIObjectData

## Prefab of the fruit to drop.
@export var fruit_prefab: PackedScene
## Max distance fruits can be dropped.  Note that units depend on whether setup as 2D or 3D.
@export var drop_distance: float = 5
## Minimum amount of fruits to drop.
@export var drop_min_amount: int = 1
## Maximum amount of fruits to drop.
@export var drop_max_amount: int = 3

## How long the tree takes to recover between drops.
@export var cooldown_window: float = 5
var is_on_cooldown: bool
var _cooldown_timer: float
## A really simple visual to show the cooldown.
@export var cooldown_display: Label

## Reference to GdPAI interactable.
@export var interactable_attribs: GdPAIInteractable
## Reference to GdPAI location data.
@export var location_data: GdPAILocationData


func _process(delta: float) -> void:
	# Simple logic monitoring when the tree is on cooldown.
	if is_on_cooldown:
		_cooldown_timer += delta
		if _cooldown_timer >= cooldown_window:
			is_on_cooldown = false
			_cooldown_timer = 0

	# Simple logic for cooldown display.
	if is_on_cooldown:
		cooldown_display.text = "%.2f" % (cooldown_window - _cooldown_timer)
	else:
		cooldown_display.text = ""


## Spawns a randomized number of fruit in a radius around the tree.
func drop_fruit():
	is_on_cooldown = true
	var amt_to_drop: int = randi_range(drop_min_amount, drop_max_amount)
	for i in range(amt_to_drop):
		var fruit_obj = fruit_prefab.instantiate()
		get_tree().root.add_child(fruit_obj)
		# Placement depends on if the fruit is 2D or 3D
		if fruit_obj is Node2D:
			fruit_obj.global_position = (
				location_data.position
				+ Vector2(
					randf_range(-drop_distance, drop_distance),
					randf_range(0.5 * drop_distance, drop_distance)  # For top-down effect.
				)
			)
		elif fruit_obj is Node3D:
			fruit_obj.global_position = (
				location_data.position
				+ Vector3(
					randf_range(-drop_distance, drop_distance),
					1,
					randf_range(-drop_distance, drop_distance)
				)
			)


# Override
func get_group_labels():
	# Make sure to add a group label for this class of data.
	return ["SampleFruitTreeObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	# Overwrite the get_provided_actions function to serve any actions that become possible because
	# this object exists out in the world.
	var shake_tree_action: SampleShakeTreeAction = SampleShakeTreeAction.new(
		location_data, interactable_attribs, self
	)
	return [shake_tree_action]


# Override
func copy_for_simulation() -> GdPAIObjectData:
	# Make sure to replace <GdPAIObjectData> with the subclass name, and to duplicate any new properties.
	var new_data: SampleFruitTreeObject = SampleFruitTreeObject.new()
	assign_uid_and_entity(new_data)
	new_data.is_on_cooldown = is_on_cooldown
	return new_data
