extends Node
## Drives an [AnimatedSprite2D] based on a [RigidBody2D] entity's velocity.
## Plays "Run" while moving and "Idle" when still. Flips the sprite horizontally
## to face the direction of travel.


## The entity whose velocity drives the animation.
@export var entity: RigidBody2D
## The sprite to animate.
@export var animated_sprite: AnimatedSprite2D
## Velocity magnitude below which the agent is considered idle.
@export var idle_threshold: float = 16.0


func _process(_delta: float) -> void:
	if entity.linear_velocity.length() > idle_threshold:
		animated_sprite.play("Run")
	else:
		animated_sprite.play("Idle")

	if entity.linear_velocity.x < 0:
		animated_sprite.scale.x = -1
	elif entity.linear_velocity.x > 0:
		animated_sprite.scale.x = 1
