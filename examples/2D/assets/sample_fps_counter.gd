## Simple FPS counter for monitoring the performance in GdPAI demos.

extends Node

## Reference to a label for displaying fps results.
@export var display_text: Label
## What frame is running?
var frame_idx: int
## How long to count for a stress test before printing to console.
@export var frame_idx_to_report: int = 2000


func _process(delta: float) -> void:
	# Using Time to measure FPS seems more consistent than using delta.
	var time_current: float = Time.get_ticks_msec() / 1000.0

	# Display in label.
	var fps_info: String = (
		"Frame: %s\nFPS:\n    Current Engine FPS: %s\n    frames/time: %.2f"
		% [frame_idx, Engine.get_frames_per_second(), frame_idx / time_current]
	)
	display_text.text = fps_info

	# Display a report to console.
	if frame_idx == frame_idx_to_report:
		print("--------")
		print(get_tree().root.get_child(0).name)
		print("Result from frame %s to %s:" % [frame_idx - frame_idx_to_report, frame_idx])
		print("Time elapsed: %.2f secs" % [time_current])
		print("FPS based on time: %.2f" % [frame_idx / time_current])

	frame_idx += 1
