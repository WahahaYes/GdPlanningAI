extends GutTest


func test_clone_creates_independent_copy():
	var original = GdPAIBlackboard.new()
	original.set_property("health", 100)
	original.set_property("name", "TestAgent")

	var clone = original.clone_for_simulation()

	clone.set_property("health", 50)
	clone.set_property("name", "ClonedAgent")

	assert_eq(original.get_property("health"), 100, "Original health should remain unchanged")
	assert_eq(original.get_property("name"), "TestAgent", "Original name should remain unchanged")
	assert_eq(clone.get_property("health"), 50, "Clone health should be updated")
	assert_eq(clone.get_property("name"), "ClonedAgent", "Clone name should be updated")


func test_clone_preserves_all_properties():
	var original = GdPAIBlackboard.new()
	original.set_property("int_value", 42)
	original.set_property("float_value", 3.14)
	original.set_property("string_value", "hello")
	original.set_property("bool_value", true)

	var clone = original.clone_for_simulation()

	assert_eq(clone.get_property("int_value"), 42, "Integer should be cloned")
	assert_eq(clone.get_property("float_value"), 3.14, "Float should be cloned")
	assert_eq(clone.get_property("string_value"), "hello", "String should be cloned")
	assert_eq(clone.get_property("bool_value"), true, "Boolean should be cloned")


func test_multiple_clones_are_independent():
	var original = GdPAIBlackboard.new()
	original.set_property("counter", 0)

	var clone1 = original.clone_for_simulation()
	var clone2 = original.clone_for_simulation()

	clone1.set_property("counter", 10)
	clone2.set_property("counter", 20)
	original.set_property("counter", 5)

	assert_eq(original.get_property("counter"), 5, "Original should have value 5")
	assert_eq(clone1.get_property("counter"), 10, "Clone1 should have value 10")
	assert_eq(clone2.get_property("counter"), 20, "Clone2 should have value 20")
