class_name DebugTextSwitch
extends Node
## The [DebugText] switch for scenes without the 3D camera rig (the 2D examples): H hides or
## shows the labels over agents and objects, and [member show_debug_text] sets it when the
## scene starts. Labels that spawn later follow the switch. Presentation only.

## Shows the debug text. False hides it when the scene starts; code can also flip it through
## [DebugText].
@export var show_debug_text: bool = true:
	set = _set_show_debug_text


func _ready() -> void:
	# Code that hid the text before the scene loaded keeps it hidden.
	DebugText.set_shown(get_tree(), show_debug_text and DebugText.is_shown(get_tree()))
	get_tree().node_added.connect(_on_node_added)


func _unhandled_input(event: InputEvent) -> void:
	var key: InputEventKey = event as InputEventKey
	if key != null and key.pressed and not key.echo and key.keycode == KEY_H:
		DebugText.toggle(get_tree())
		get_viewport().set_input_as_handled()


func _on_node_added(node: Node) -> void:
	DebugText.apply(node)


func _set_show_debug_text(value: bool) -> void:
	show_debug_text = value
	if is_inside_tree():
		DebugText.set_shown(get_tree(), value)
