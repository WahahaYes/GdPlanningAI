extends Node
## Moves a [RigidBody2D] entity toward the next path position reported by a
## [NavigationAgent2D] each physics frame.

## The top-level entity node to move.
@export var entity: RigidBody2D
## The navigation agent driving path-finding.
@export var nav_agent: NavigationAgent2D
## Movement speed in pixels per second.
@export var speed: float = 128.0
## Distance in pixels to consider "arrived" at the next path position.
@export var arrival_threshold: float = 8.0


func _physics_process(_delta: float) -> void:
	var direction: Vector2 = nav_agent.get_next_path_position() - entity.global_position
	if direction.length() > arrival_threshold:
		entity.linear_velocity = direction.normalized() * speed
	else:
		entity.linear_velocity = Vector2.ZERO
