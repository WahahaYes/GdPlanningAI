class_name GdPAIAwaitable
extends RefCounted


static func callDeferred(obj: Object, method: String, args: Array = []) -> AwaitableCallDeferred:
	return AwaitableCallDeferred.new(obj, method, args)


class AwaitableCallDeferred:
	signal finished(result)

	func _init(obj: Object, method: String, args: Array = []) -> void:
		call_deferred("_callAndSignal", obj, method, args)

	func _callAndSignal(obj: Object, method: String, args: Array = []) -> void:
		var result = obj.callv(method, args)
		emit_signal("finished", result)
