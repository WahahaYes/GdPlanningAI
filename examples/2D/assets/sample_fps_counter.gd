## Simple FPS counter for monitoring the performance in GdPAI demos.

extends Node

## Reference to a label for displaying fps results.
@export var display_text: Label
## How long of a window to store FPS data.
@export var window_size: int = 60
## How long to count for a stress test.
@export var stress_test_num_frames: int = 2000
## Array to store fps values.
@onready var fps_window: Array[float] = []
## Array to store fps values.
@onready var long_fps_window: Array[float] = []
## What frame is running?
var frame_idx: int


func _process(delta: float) -> void:
	var fps_current: float = Engine.get_frames_per_second()
	fps_window.append(fps_current)
	long_fps_window.append(fps_current)
	if fps_window.size() > window_size:
		fps_window.pop_front()

	var sum: float = 0
	for n in fps_window:
		sum += n  # Ensure n is treated as a float for accurate division
	var mean: float = sum / fps_window.size()
	var spike: float = fps_window.min()

	var variance: float = 0
	for n in fps_window:
		variance += pow(n - mean, 2)
	variance /= fps_window.size()

	frame_idx += 1

	var fps_info: String = (
		"Frame: %s\nFPS:\n    current: %.2f\n    average: %.2f\n    std dev: %.2f\n    low spike: %.2f"
		% [frame_idx, fps_current, mean, sqrt(variance), spike]
	)
	display_text.text = fps_info

	if frame_idx == stress_test_num_frames:
		sum = 0
		for n in long_fps_window:
			sum += n  # Ensure n is treated as a float for accurate division
		mean = sum / long_fps_window.size()
		spike = long_fps_window.min()

		variance = 0
		for n in long_fps_window:
			variance += pow(n - mean, 2)
		variance /= long_fps_window.size()

		print("Result after %s frames:" % stress_test_num_frames)
		fps_info = (
			"Frame: %s\nFPS:\n    current: %.2f\n    average: %.2f\n    std dev: %.2f\n    low spike: %.2f"
			% [frame_idx, fps_current, mean, sqrt(variance), spike]
		)
		print(fps_info)
