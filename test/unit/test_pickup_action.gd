extends GutTest
## PickupAction provisions the held_item binding while its item lives, and nothing once freed.


## Builds a holdable item with [param item_id], optionally in the Food group.
func _make_item(item_id: String, edible: bool = false) -> HoldableObject:
	var item: HoldableObject = HoldableObject.new()
	item.item_id = item_id
	if edible:
		item.add_to_group("Food")
	add_child_autofree(item)
	return item


## A live item provisions its held_item binding.
func test_get_provisions_binds_held_item() -> void:
	var action: PickupAction = PickupAction.new(null, null, _make_item("wood"))
	var provs: Array[ProvisionSpec] = action.get_provisions()
	assert_eq(provs.size(), 1, "A live item provisions exactly the held_item binding")
	assert_eq(provs[0].to_bridge_dict().get("value"), "wood", "Binding carries the item id")


## A live food item also provisions the is_food fact.
func test_get_provisions_adds_is_food_for_food() -> void:
	var action: PickupAction = PickupAction.new(null, null, _make_item("apple", true))
	var provs: Array[ProvisionSpec] = action.get_provisions()
	assert_eq(provs.size(), 2, "Food provisions held_item plus is_food")
	assert_eq(provs[1].to_bridge_dict().get("fact_name"), "is_food", "Second provision is is_food")


## A freed item provisions nothing instead of erroring on the dereference.
func test_get_provisions_returns_empty_for_freed_item() -> void:
	var item: HoldableObject = HoldableObject.new()
	item.item_id = "wood"
	var action: PickupAction = PickupAction.new(null, null, item)
	item.free()
	var provs: Array[ProvisionSpec] = action.get_provisions()
	assert_eq(provs.size(), 0, "A freed item provisions nothing")


## Bridge serialization of a freed-item action provisions nothing instead of erroring.
func test_bridge_serialize_freed_pickup_provisions_nothing() -> void:
	var item: HoldableObject = HoldableObject.new()
	item.item_id = "wood"
	var action: PickupAction = PickupAction.new(null, null, item)
	item.free()
	var bridge: GdPAIRustBridge = GdPAIRustBridge.new()
	var extracted: Array[Dictionary] = bridge.serialize_actions([action])
	assert_eq(extracted[0]["provisions"], [], "Freed item serializes to no provisions")
