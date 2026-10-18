extends Node3D
## Shows the agent's [code]held_item[/code] as a prop bobbing above its head. Presentation only:
## it polls the blackboard (which has no change signal) and never writes back.

## Raw dug-up potato visual.
const RAW_SCENE: PackedScene = preload("res://examples/source_3d/prefabs/held_potato_raw_3d.tscn")
## Cooked potato visual.
const COOKED_SCENE: PackedScene = preload(
	"res://examples/source_3d/prefabs/held_potato_cooked_3d.tscn"
)
## Single-log visual for carried wood.
const WOOD_SCENE: PackedScene = preload("res://examples/source_3d/prefabs/held_wood_3d.tscn")

## The agent whose held_item property drives the visual.
@export var gdpai_agent: GdPAIAgent
## Maps held_item ids to prop scenes. Left empty, it gets the potato pair and wood.
@export var held_visuals: Dictionary = {}
## Vertical bob amplitude in meters.
@export var hover_amplitude: float = 0.08
## Full bob cycles per second.
@export var hover_frequency: float = 1.0
## Slow turntable speed in radians per second.
@export var spin_speed: float = 1.2

## Currently displayed held_item id.
var _current_id: String = ""
## Currently instanced visual, if any.
var _visual: Node3D = null
## Base local height of the visual before bob is applied.
var _base_y: float = 0.0
## Accumulated time driving the bob.
var _time: float = 0.0


## Fills the default visual map when the editor left it empty.
func _ready() -> void:
	if held_visuals.is_empty():
		held_visuals = {
			"potato": RAW_SCENE,
			"cooked_potato": COOKED_SCENE,
			"wood": WOOD_SCENE,
		}


## Polls the blackboard and swaps the visual when held_item changes.
func _process(delta: float) -> void:
	_time += delta
	if is_instance_valid(_visual):
		_visual.position.y = _base_y + sin(_time * TAU * hover_frequency) * hover_amplitude
		_visual.rotation.y += delta * spin_speed
	if not is_instance_valid(gdpai_agent):
		return
	var bb: GdPAIBlackboard = gdpai_agent.blackboard
	if bb == null:
		return
	var held_raw: Variant = bb.get_property("held_item")
	var held_id: String = ""
	if held_raw != null:
		held_id = str(held_raw)
	if held_id == _current_id:
		return
	_show(held_id)


## Replaces the socket child with the visual for [param held_id].
func _show(held_id: String) -> void:
	_current_id = held_id
	if is_instance_valid(_visual):
		_visual.queue_free()
		_visual = null
	if held_id.is_empty() or not held_visuals.has(held_id):
		return
	var scene: PackedScene = held_visuals[held_id] as PackedScene
	if scene == null or not scene.can_instantiate():
		return
	_visual = scene.instantiate() as Node3D
	if _visual == null:
		return
	_base_y = _visual.position.y
	add_child(_visual)
