class_name NavigatingAction
extends Action
## Base class for actions that involve navigating an agent to a destination using
## a [NavigationAgent2D] or [NavigationAgent3D].
##[br]
##[br]
## Provides shared navigation utilities: arrival thresholds and nav-agent discovery.
## Extend this class (or [SpatialAction]) rather than [Action] directly when your
## action requires the agent to physically move somewhere to piggyback off of Godot's
## navigation system.

## Arrival distance threshold for 2D navigation (pixels).
const ARRIVAL_THRESHOLD_2D: float = 8.0
## Arrival distance threshold for 3D navigation (meters).
const ARRIVAL_THRESHOLD_3D: float = 0.1


## Returns the [NavigationAgent2D] or [NavigationAgent3D] child of [param entity],
## or [code]null[/code] if neither is present.
## Asserts if both are present simultaneously.
static func _find_nav_agent(entity: Node) -> Node:
	var nav_2d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent2D)
	var nav_3d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent3D)
	assert(
		nav_2d == null or nav_3d == null,
		"Entity should not have both a NavigationAgent2D and a NavigationAgent3D."
	)
	if nav_2d != null:
		return nav_2d
	return nav_3d
