class_name PropertyUpdater
extends RefCounted
## Base class for updating agent blackboard properties over time.
## Extend this class to create custom property update logic.


## Called every frame to update agent properties.
## Override this method to implement custom property update logic.
func update_properties(_agent: GdPAIAgent, _delta: float) -> void:
	pass


## Called when the property updater is first initialized.
## Override this method to set up initial property values.
func initialize(_agent: GdPAIAgent) -> void:
	pass
