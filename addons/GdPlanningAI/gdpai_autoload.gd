extends Node
## This is an autoload node for easy reference to the GdPlanningAI addon.
## Right now, it just serves as a reference used by the debugger for game
## initialization to clear up the debugger info between runs.


## Reads plugin.cfg and applies the configured log level to the Rust engine,
## then clears debugger state for the new run.
func _ready() -> void:
	_apply_log_level()
	EngineDebugger.send_message("gdplanningai:clear_state", [])


func _apply_log_level() -> void:
	var config := ConfigFile.new()
	var err := config.load("res://addons/GdPlanningAI/plugin.cfg")
	if err != OK:
		push_warning("GdPlanningAI: could not load plugin.cfg (error %d)" % err)
		return
	var level: int = config.get_value("configuration", "log_level", 2)
	var engine := RustPlanningEngine.new()
	engine.set_log_level(level)
