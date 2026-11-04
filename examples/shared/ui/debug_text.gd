class_name DebugText
extends RefCounted
## One switch for the examples' debug text: the on-screen controls guide and the labels over
## agents and objects. Those nodes join [constant GROUP]; [method set_shown] shows or hides
## them all. Call it from code before or after a scene loads:
## [code]DebugText.set_shown(get_tree(), false)[/code]. In the 3D examples the camera rig drives
## the same switch (its [code]show_debug_text[/code] export and H key) and applies it to labels
## that spawn later. The switch touches a node only when it flips or the node spawns, so code
## can still show or hide one label through its own [code]visible[/code].

## Group of nodes the switch shows and hides.
const GROUP: StringName = &"debug_text"

## Root metadata key holding the switch, so it lives as long as the tree.
const META_KEY: StringName = &"debug_text_shown"


## Shows or hides every node in [constant GROUP] in [param tree].
static func set_shown(tree: SceneTree, value: bool) -> void:
	tree.root.set_meta(META_KEY, value)
	for node in tree.get_nodes_in_group(GROUP):
		apply(node)


## Whether debug text is shown in [param tree] (true until something hides it).
static func is_shown(tree: SceneTree) -> bool:
	return tree.root.get_meta(META_KEY, true)


## Flips the switch.
static func toggle(tree: SceneTree) -> void:
	set_shown(tree, not is_shown(tree))


## Applies the switch to one node if it is debug text, e.g. one that just spawned.
static func apply(node: Node) -> void:
	if node.is_in_group(GROUP) and node.is_inside_tree():
		node.set("visible", is_shown(node.get_tree()))
