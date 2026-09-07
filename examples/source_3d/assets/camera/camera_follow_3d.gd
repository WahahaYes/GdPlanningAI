extends Node3D
## Smooth follow rig for the 3D demo camera.
##
## Position lerps toward [member target] each frame so the [Camera3D]
## riding the [SpringArm3D] child tracks the agent without jitter.
## Pitch/yaw live on the SpringArm; zoom via its spring length.
## Drag with right/middle mouse to orbit (pitch clamps), wheel to zoom,
## R resets the view. No InputMap actions required.

## The node to follow (the Agent1 CharacterBody3D).
@export var target: Node3D
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


## Resolves the default agent when no target is assigned.
func _ready() -> void:
	if target == null:
		var scene: Node = get_tree().current_scene
		if scene != null:
			target = scene.get_node_or_null("Agent1") as Node3D
	if target == null:
		target = get_node_or_null("../Agent1") as Node3D


## Tracks the target with frame-rate independent smoothing.
func _process(delta: float) -> void:
	if target == null:
		return
	var goal: Vector3 = target.global_position + Vector3(0.0, height, 0.0)
	var rate: float = clampf(follow_speed * delta, 0.0, 1.0)
	global_position = global_position.lerp(goal, rate)


## Handles orbit drag, wheel zoom, and the reset key.
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
		if key.pressed and not key.echo and key.keycode == KEY_R:
			reset_view()
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


## Returns the SpringArm3D child driving the camera.
func _arm() -> SpringArm3D:
	return get_node_or_null("SpringArm3D") as SpringArm3D
