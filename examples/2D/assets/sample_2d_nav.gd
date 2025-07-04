## Simple movement from NavigationAgent2D used to power the GdPAI 2D demo.

extends Node

## The top-level node of the agent to move.
@export var entity: Node2D
## Reference to the navigation agent.
@export var nav_agent: NavigationAgent2D
## How fast the agent moves.
@export var speed: float = 2


func _physics_process(delta: float) -> void:
	var direction: Vector2 = nav_agent.get_next_path_position() - entity.global_position
	if direction.length() > 1:
		var offset: Vector2 = direction.normalized() * speed * delta
		entity.global_position += offset
