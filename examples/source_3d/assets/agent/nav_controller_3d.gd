extends Node
## Moves a [CharacterBody3D] along its [NavigationAgent3D] path. [GoToAction] only sets the
## target; this turns the path into motion.[br]
## Horizontal velocity eases toward [member speed] and, with avoidance enabled, goes through the
## navigation server; Y is left to gravity. Avoidance only separates agents: campfires and wood
## piles carve the navmesh, so paths already go around them.

## The entity to move.
@export var entity: CharacterBody3D
## The navigation agent driving path-finding.
@export var nav_agent: NavigationAgent3D
## Movement speed in meters per second.
@export var speed: float = 3.0
## Horizontal easing rate toward the desired velocity.
@export var acceleration: float = 12.0
## Yaw turn speed in radians per second for facing movement.
@export var turn_speed: float = 10.0
## Downward acceleration applied while airborne.
@export var gravity: float = 9.8
## Distance in meters to consider "arrived" at the next path position.
@export var arrival_threshold: float = 0.2


## Connects the avoidance result when avoidance is on.
func _ready() -> void:
	if nav_agent != null and nav_agent.avoidance_enabled:
		nav_agent.max_speed = speed
		nav_agent.velocity_computed.connect(_move)


func _physics_process(delta: float) -> void:
	if entity == null or nav_agent == null:
		return

	var to: Vector3 = nav_agent.get_next_path_position() - entity.global_position
	to.y = 0.0
	var desired: Vector3 = Vector3.ZERO
	if to.length() > arrival_threshold:
		desired = to.normalized() * speed
	var rate: float = clampf(acceleration * delta, 0.0, 1.0)
	var steer: Vector3 = Vector3(
		lerpf(entity.velocity.x, desired.x, rate), 0.0, lerpf(entity.velocity.z, desired.z, rate)
	)
	if nav_agent.avoidance_enabled:
		# The navigation server answers with velocity_computed, which calls _move.
		nav_agent.velocity = steer
	else:
		_move(steer)


## Moves the entity with [param horizontal] velocity, applying gravity and facing.
func _move(horizontal: Vector3) -> void:
	var delta: float = get_physics_process_delta_time()
	var current: Vector3 = entity.velocity
	current.x = horizontal.x
	current.z = horizontal.z
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
