extends Node
## This is an autoload node for easy reference to the GdPlanningAI addon.
## Right now, it just serves as a reference used by the debugger for game
## initialization to clear up the debugger info between runs.


## Clears debugger state when game starts by messaging the debugger.
func _ready() -> void:
	print("Clearing debugger state")
	EngineDebugger.send_message("gdplanningai:clear_state", [])
