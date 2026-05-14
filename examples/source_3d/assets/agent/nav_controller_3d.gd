extends Node
## Moves a [CharacterBody3D] entity toward the next path position reported by a
## [NavigationAgent3D] each physics frame.

## The entity to move.
@export var entity: CharacterBody3D
## The navigation agent driving path-finding.
@export var nav_agent: NavigationAgent3D
## Movement speed in meters per second.
@export var speed: float = 3.0
## Distance in meters to consider "arrived" at the next path position.
@export var arrival_threshold: float = 0.1


## Advances the entity toward the navigation path target.
func _physics_process(_delta: float) -> void:
	if entity == null or nav_agent == null:
		return

	var direction: Vector3 = nav_agent.get_next_path_position() - entity.global_position
	if direction.length() > arrival_threshold:
		entity.velocity = direction.normalized() * speed
	else:
		entity.velocity = Vector3.ZERO
	entity.move_and_slide()
