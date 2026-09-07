extends Node
## Moves a [CharacterBody3D] entity toward the next path position reported by a
## [NavigationAgent3D] each physics frame.

## The entity to move.
@export var entity: CharacterBody3D
## The navigation agent driving path-finding.
@export var nav_agent: NavigationAgent3D
## Movement speed in meters per second.
@export var speed: float = 3.0
## Horizontal acceleration toward the desired velocity.
@export var acceleration: float = 12.0
## Yaw turn speed in radians per second for facing movement.
@export var turn_speed: float = 10.0
## Downward acceleration applied while airborne.
@export var gravity: float = 9.8
## Distance in meters to consider "arrived" at the next path position.
@export var arrival_threshold: float = 0.2


## Advances the entity toward the navigation path target with smoothing,
## facing, and gravity. Horizontal steering follows [NavigationAgent3D]
## while Y stays physics-driven so the body rests on the ground.
func _physics_process(delta: float) -> void:
	if entity == null or nav_agent == null:
		return

	var to: Vector3 = nav_agent.get_next_path_position() - entity.global_position
	to.y = 0.0
	var desired: Vector3 = Vector3.ZERO
	if to.length() > arrival_threshold:
		desired = to.normalized() * speed
	var current: Vector3 = entity.velocity
	var rate: float = clampf(acceleration * delta, 0.0, 1.0)
	current.x = lerpf(current.x, desired.x, rate)
	current.z = lerpf(current.z, desired.z, rate)
	if not entity.is_on_floor():
		current.y -= gravity * delta
	elif current.y < 0.0:
		current.y = 0.0
	entity.velocity = current
	entity.move_and_slide()
	_face_movement(delta)


## Rotates the entity yaw toward its horizontal velocity.
func _face_movement(delta: float) -> void:
	var flat: Vector3 = entity.velocity
	flat.y = 0.0
	if flat.length() < 0.2:
		return
	var target_yaw: float = atan2(flat.x, flat.z)
	entity.rotation.y = lerp_angle(
		entity.rotation.y, target_yaw, clampf(turn_speed * delta, 0.0, 1.0)
	)
