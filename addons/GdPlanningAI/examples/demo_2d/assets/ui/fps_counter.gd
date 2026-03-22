extends Node
## Simple FPS counter for monitoring performance in GdPAI demos.


## Label node to display FPS information.
@export var display_text: Label

var _frame_idx: int = 0


func _process(_delta: float) -> void:
	var time_current: float = Time.get_ticks_msec() / 1000.0
	display_text.text = (
		"FPS: %s\nAvg: %.1f"
		% [Engine.get_frames_per_second(), _frame_idx / max(time_current, 0.001)]
	)
	_frame_idx += 1
