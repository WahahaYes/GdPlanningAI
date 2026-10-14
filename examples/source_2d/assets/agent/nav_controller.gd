extends Node
## Moves a [RigidBody2D] entity toward the next path position reported by a
## [NavigationAgent2D] each physics frame. With avoidance enabled on the agent,
## the steering velocity goes through the navigation server and the safe velocity
## it returns is applied instead.[br]
##[br]
## This is movement, not planning. [GoToAction] only sets the nav agent's target; this
## script is what turns the path into motion, so the same planning code runs unchanged
## whatever body the agent has.[br]
##[br]
## Steering goes through avoidance for a reason. With several agents walking to the
## same wood pile or campfire, plain "move toward the next path point" makes them
## clump and shove each other. The 2D agent bodies have physics collisions turned off
## (collision layer and mask 0), so nothing physical keeps them apart. Instead the wanted
## velocity is handed to [member NavigationAgent2D.velocity]; the navigation server
## blends it with neighbouring agents' velocities and answers on
## [signal NavigationAgent2D.velocity_computed] with a safe velocity, which [method _move]
## applies. Without avoidance enabled the wanted velocity is applied directly.[br]
##[br]
## The avoidance [code]radius[/code] set on each agent scene is the body's collision
## shape plus about 10 percent: 18 for the 16 pixel circle on [code]agent_2d.tscn[/code]
## and 36 for the 64 pixel square on [code]agent.tscn[/code]. A radius smaller than the
## body lets agents overlap; a much larger one makes them keep too far apart to crowd
## through a doorway. [member NavigationAgent2D.max_speed] is set from [member speed] so
## the server never asks for more than the agent can move.

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


## Computes the wanted velocity toward the next path point and routes it through avoidance.
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
