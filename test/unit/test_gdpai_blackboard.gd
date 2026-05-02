extends GutTest


func test_property_basic_operations():
	var bb = GdPAIBlackboard.new()

	bb.set_property("health", 100)
	assert_true(bb.has_property("health"), "Should have property after setting")
	assert_eq(bb.get_property("health"), 100, "Should retrieve set property")

	bb.erase_property("health")
	assert_false(bb.has_property("health"), "Should not have property after erasing")
	assert_eq(bb.get_property("health"), null, "Erased property should return null")


func test_property_type_preservation():
	var bb = GdPAIBlackboard.new()

	bb.set_property("int_val", 42)
	bb.set_property("float_val", 3.14)
	bb.set_property("string_val", "hello")
	bb.set_property("bool_val", true)
	bb.set_property("array_val", [1, 2, 3])

	assert_eq(bb.get_property("int_val"), 42)
	assert_eq(bb.get_property("float_val"), 3.14)
	assert_eq(bb.get_property("string_val"), "hello")
	assert_eq(bb.get_property("bool_val"), true)
	assert_eq(bb.get_property("array_val"), [1, 2, 3])


func test_property_overwrite():
	var bb = GdPAIBlackboard.new()

	bb.set_property("value", 10)
	assert_eq(bb.get_property("value"), 10)

	bb.set_property("value", 20)
	assert_eq(bb.get_property("value"), 20, "Should overwrite existing property")


func test_get_dict_and_set_dict():
	var bb = GdPAIBlackboard.new()
	bb.set_property("a", 1)
	bb.set_property("b", 2)
	bb.set_property("c", 3)

	var dict = bb.get_dict()
	assert_eq(dict["a"], 1, "Dict should contain property a")
	assert_eq(dict["b"], 2, "Dict should contain property b")
	assert_eq(dict["c"], 3, "Dict should contain property c")

	var bb2 = GdPAIBlackboard.new()
	bb2.set_dict(dict)
	assert_eq(bb2.get_property("a"), 1, "Should restore property a from dict")
	assert_eq(bb2.get_property("b"), 2, "Should restore property b from dict")
	assert_eq(bb2.get_property("c"), 3, "Should restore property c from dict")


func test_set_dict_clears_existing():
	var bb = GdPAIBlackboard.new()
	bb.set_property("old_prop", "old")

	bb.set_dict({"new_prop": "new"})

	assert_false(bb.has_property("old_prop"), "Old property should be cleared")
	assert_true(bb.has_property("new_prop"), "New property should exist")


func test_missing_property_returns_null():
	var bb = GdPAIBlackboard.new()

	assert_eq(bb.get_property("nonexistent"), null, "Missing property returns null")
	assert_false(bb.has_property("nonexistent"), "Missing property has_property false")


func test_erase_missing_property_safe():
	var bb = GdPAIBlackboard.new()

	bb.erase_property("nonexistent")
	assert_false(bb.has_property("nonexistent"), "Erasing missing property is safe")


# Helper to create test object data nodes
class TestObjectData:
	extends GdPAIObjectData
	var _extra_groups: Array[String] = []
	var _sim_props: Dictionary = {}

	func _init(extra_groups: Array[String] = [], sim_props: Dictionary = {}):
		_extra_groups = extra_groups
		_sim_props = sim_props
		super._init()

	func get_group_labels() -> Array[String]:
		var labels = super.get_group_labels()
		labels.append_array(_extra_groups)
		return labels

	func get_sim_properties() -> Dictionary:
		return _sim_props


func test_gdpai_objects_property_creates_proxies():
	var bb = GdPAIBlackboard.new()

	var obj1 = TestObjectData.new(["enemy", "mobile"], {"health": 100})
	var obj2 = TestObjectData.new(["ally"], {"health": 50})

	add_child_autofree(obj1)
	add_child_autofree(obj2)
	await get_tree().process_frame

	bb.set_property("GDPAI_OBJECTS", [obj1, obj2])

	var enemies = bb.get_proxies_in_group("enemy")
	assert_eq(enemies.size(), 1, "Should find 1 enemy")

	var allies = bb.get_proxies_in_group("ally")
	assert_eq(allies.size(), 1, "Should find 1 ally")


func test_get_proxy_in_group_returns_first():
	var bb = GdPAIBlackboard.new()

	var obj1 = TestObjectData.new(["enemy"], {"id": 1})
	var obj2 = TestObjectData.new(["enemy"], {"id": 2})

	add_child_autofree(obj1)
	add_child_autofree(obj2)
	await get_tree().process_frame

	bb.set_property("GDPAI_OBJECTS", [obj1, obj2])

	var proxy = bb.get_proxy_in_group("enemy")
	assert_not_null(proxy, "Should return a proxy")


func test_get_proxy_in_group_returns_null_when_not_found():
	var bb = GdPAIBlackboard.new()

	var obj1 = TestObjectData.new(["enemy"], {})
	add_child_autofree(obj1)
	await get_tree().process_frame

	bb.set_property("GDPAI_OBJECTS", [obj1])

	var proxy = bb.get_proxy_in_group("ally")
	assert_null(proxy, "Should return null when group not found")


func test_get_object_for_returns_proxy():
	var bb = GdPAIBlackboard.new()

	var obj1 = TestObjectData.new(["test"], {"value": 42})
	add_child_autofree(obj1)
	await get_tree().process_frame

	bb.set_property("GDPAI_OBJECTS", [obj1])

	var proxy = bb.get_object_for(obj1)
	assert_not_null(proxy, "Should return proxy for the node")

	if proxy != null:
		assert_eq(proxy.get_property("value"), 42, "Proxy should have sim properties")


func test_clone_preserves_object_proxies():
	var bb = GdPAIBlackboard.new()

	var obj1 = TestObjectData.new(["enemy"], {"health": 100})
	add_child_autofree(obj1)
	await get_tree().process_frame

	bb.set_property("GDPAI_OBJECTS", [obj1])
	bb.set_property("turn", 1)

	var clone = bb.clone_for_simulation()

	var original_enemies = bb.get_proxies_in_group("enemy")
	var cloned_enemies = clone.get_proxies_in_group("enemy")

	assert_eq(original_enemies.size(), 1, "Original should have 1 enemy")
	assert_eq(cloned_enemies.size(), 1, "Clone should have 1 enemy")

	# Mutate clone's proxy
	if cloned_enemies.size() > 0:
		cloned_enemies[0].set_property("health", 50)

	# Original should be unchanged
	if original_enemies.size() > 0:
		assert_eq(
			original_enemies[0].get_property("health"),
			100,
			"Original proxy should remain unchanged"
		)
