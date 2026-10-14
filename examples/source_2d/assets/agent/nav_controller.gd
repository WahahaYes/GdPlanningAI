extends Node
## Moves a [RigidBody2D] along its [NavigationAgent2D] path. [GoToAction] only sets the target;
## this turns the path into motion.[br]
## The 2D bodies do not collide, so with avoidance enabled the wanted velocity goes through the
## navigation server and the safe velocity it returns is applied. Each scene's avoidance radius
## is the collision shape plus about 10%.

## The top-level entity node to move.
@export var entity: RigidBody2D
## The navigation agent driving path-finding.
@export var nav_agent: NavigationAgent2D
## Movement speed in pixels per second.
@export var speed: float = 128.0
## Distance in pixels to consider "arrived" at the next path position.
@export var arrival_threshold: float = 8.0


## Connects the avoidance result when avoidance is on.
func _ready() -> void:
	if nav_agent != null and nav_agent.avoidance_enabled:
		nav_agent.max_speed = speed
		nav_agent.velocity_computed.connect(_move)


func _physics_process(_delta: float) -> void:
	var direction: Vector2 = nav_agent.get_next_path_position() - entity.global_position
	# Zero inside the arrival threshold so the agent settles instead of jittering on the point.
	var steer: Vector2 = Vector2.ZERO
	if direction.length() > arrival_threshold:
		steer = direction.normalized() * speed
	if nav_agent.avoidance_enabled:
		# The navigation server answers with velocity_computed, which calls _move.
		nav_agent.velocity = steer
	else:
		_move(steer)


## Applies [param velocity] to the entity.
func _move(velocity: Vector2) -> void:
	entity.linear_velocity = velocity
