extends Node
## Simple animation script used to power the GdPAI 2D demo.

## Reference to the agent's entity
@export var entity: RigidBody2D
## Reference to the sprite.
@export var animated_sprite: AnimatedSprite2D
## Velocity threshold for sprite animation.
@export var idle_threshold: float = 1


func _process(_delta: float) -> void:
	# Animations.
	if entity.linear_velocity.length() > idle_threshold:
		animated_sprite.play("Run")
	else:
		animated_sprite.play("Idle")
	# Flip the agent to face their movement direction.
	if entity.linear_velocity.x < 0:
		animated_sprite.scale.x = -1
	elif entity.linear_velocity.x > 0:
		animated_sprite.scale.x = 1
