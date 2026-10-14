extends Node3D
## Smooth follow rig for the 3D demo camera.
##
## Position lerps toward [member target] each frame so the [Camera3D]
## riding the [SpringArm3D] child tracks the agent without jitter.
## Pitch/yaw live on the SpringArm; zoom via its spring length.
## Drag with right/middle mouse to orbit (pitch clamps), wheel to zoom,
## R resets the view, Tab / Shift+Tab switch between the scene's agents.
## A small controls guide is pinned top-left. No InputMap actions required.
##
## Presentation only: it observes agents and never affects planning. Input is read
## directly so the example runs without editing the project's InputMap; a real game would
## bind actions instead. The large scene's spawner assigns [member target] to a spawned
## agent at startup, which is why the exported target may be empty in the scene.

## Controls guide text, below the "Following" line.
const CONTROLS_TEXT: String = "Right/middle drag: orbit\nWheel: zoom\nR: reset view"
## Guide line for agent switching, shown only when there is more than one agent.
const SWITCH_TEXT: String = "Tab / Shift+Tab: switch agent"

## The node to follow (the Agent1 CharacterBody3D).
@export var target: Node3D:
	set = _set_target
## Height above the target origin.
@export var height: float = 1.2
## Position catch-up speed.
@export var follow_speed: float = 5.0
## Radians of orbit per pixel of mouse drag.
@export var mouse_sensitivity: float = 0.005
## Lowest pitch (looking down steeply) in degrees.
@export var min_pitch_deg: float = -75.0
## Highest pitch (near level) in degrees.
@export var max_pitch_deg: float = -15.0
## Default arm yaw restored by the reset key.
@export var default_yaw: float = -0.78
## Default arm pitch restored by the reset key.
@export var default_pitch: float = -0.9
## Default arm length restored by the reset key.
@export var default_length: float = 8.0
## Minimum spring arm length when zooming in.
@export var min_length: float = 4.0
## Maximum spring arm length when zooming out.
@export var max_length: float = 14.0
## Shows the controls guide in the top-left corner.
@export var show_controls: bool = true

## Controls guide label, when shown.
var _controls_label: Label


## Resolves the default agent when no target is assigned and builds the guide.
func _ready() -> void:
	if show_controls:
		_build_controls()
	if target == null:
		var scene: Node = get_tree().current_scene
		if scene != null:
			target = scene.get_node_or_null("Agent1") as Node3D
	if target == null:
		target = get_node_or_null("../Agent1") as Node3D
	_update_controls()


## Tracks the target with frame-rate independent smoothing.
func _process(delta: float) -> void:
	if target == null:
		return
	var goal: Vector3 = target.global_position + Vector3(0.0, height, 0.0)
	var rate: float = clampf(follow_speed * delta, 0.0, 1.0)
	global_position = global_position.lerp(goal, rate)


## Handles orbit drag, wheel zoom, the reset key, and agent switching.
func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		var motion: InputEventMouseMotion = event as InputEventMouseMotion
		var held: int = motion.button_mask & (MOUSE_BUTTON_MASK_RIGHT | MOUSE_BUTTON_MASK_MIDDLE)
		if held != 0:
			orbit_pixels(motion.relative.x, motion.relative.y)
	elif event is InputEventMouseButton:
		_handle_wheel(event as InputEventMouseButton)
	elif event is InputEventKey:
		var key: InputEventKey = event as InputEventKey
		if not key.pressed or key.echo:
			return
		if key.keycode == KEY_R:
			reset_view()
			get_viewport().set_input_as_handled()
		elif key.keycode == KEY_TAB:
			cycle_target(-1 if key.shift_pressed else 1)
			get_viewport().set_input_as_handled()


## Applies wheel zoom from a mouse button event.
func _handle_wheel(mb: InputEventMouseButton) -> void:
	if not mb.pressed:
		return
	if mb.button_index == MOUSE_BUTTON_WHEEL_UP:
		zoom_steps(-1.0)
		get_viewport().set_input_as_handled()
	elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN:
		zoom_steps(1.0)
		get_viewport().set_input_as_handled()


## Rotates the arm by a mouse drag delta in pixels. Yaw is free, pitch clamps.
func orbit_pixels(dx: float, dy: float) -> void:
	var arm: SpringArm3D = _arm()
	if arm == null:
		return
	var yaw: float = arm.rotation.y - dx * mouse_sensitivity
	var pitch: float = clampf(
		arm.rotation.x - dy * mouse_sensitivity,
		deg_to_rad(min_pitch_deg),
		deg_to_rad(max_pitch_deg)
	)
	arm.rotation = Vector3(pitch, yaw, 0.0)


## Zooms the arm in (negative steps) or out while clamped to limits.
func zoom_steps(steps: float) -> void:
	var arm: SpringArm3D = _arm()
	if arm == null:
		return
	arm.spring_length = clampf(arm.spring_length + steps, min_length, max_length)


## Restores the default orbit pose and zoom.
func reset_view() -> void:
	var arm: SpringArm3D = _arm()
	if arm == null:
		return
	arm.rotation = Vector3(default_pitch, default_yaw, 0.0)
	arm.spring_length = default_length


## Follows the next (or previous, for negative [param step]) agent in scene order.
func cycle_target(step: int) -> void:
	var bodies: Array[Node3D] = follow_candidates()
	if bodies.is_empty():
		return
	var index: int = bodies.find(target)
	if index < 0:
		target = bodies[0]
		return
	target = bodies[posmod(index + step, bodies.size())]


## Returns the bodies of every planning agent in the scene, in tree order.
func follow_candidates() -> Array[Node3D]:
	var bodies: Array[Node3D] = []
	var scene: Node = get_tree().current_scene if get_tree().current_scene != null else get_parent()
	if scene == null:
		return bodies
	for agent in scene.find_children("*", "GdPAIAgent", true, false):
		var body: Node3D = agent.get_parent() as Node3D
		if body != null and not bodies.has(body):
			bodies.append(body)
	return bodies


## Keeps the guide's "Following" line current however the target changes.
func _set_target(value: Node3D) -> void:
	target = value
	_update_controls()


## Pins the controls guide to the top-left corner on its own canvas layer.
func _build_controls() -> void:
	var layer: CanvasLayer = CanvasLayer.new()
	layer.name = "ControlsGuide"
	add_child(layer)
	_controls_label = Label.new()
	_controls_label.position = Vector2(12.0, 10.0)
	_controls_label.add_theme_color_override("font_color", Color(1.0, 1.0, 1.0, 0.9))
	_controls_label.add_theme_color_override("font_outline_color", Color(0.0, 0.0, 0.0, 0.8))
	_controls_label.add_theme_constant_override("outline_size", 4)
	_controls_label.add_theme_font_size_override("font_size", 14)
	_controls_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	layer.add_child(_controls_label)


## Refreshes the guide with the agent currently followed. The switch line only
## appears when the scene has more than one agent to follow.
func _update_controls() -> void:
	if _controls_label == null or not is_inside_tree():
		return
	var following: String = String(target.name) if target != null else "none"
	var text: String = "Following: %s\n%s" % [following, CONTROLS_TEXT]
	if follow_candidates().size() > 1:
		text += "\n" + SWITCH_TEXT
	_controls_label.text = text


## Returns the SpringArm3D child driving the camera.
func _arm() -> SpringArm3D:
	return get_node_or_null("SpringArm3D") as SpringArm3D
