# Agent Skin + Animation (working plan)

Date: 2026-09-07. Status: DONE — 5/5 skin + 6/6 carry + 4/4 showcase tests pass, failure set identical to HEAD, lint clean.

## Redesign (same day): runtime skin instancing

- Dropped `skins/knight_3d.tscn`. `skin_scene` now points at a character `.glb` directly; the script instances it as `Body`, creates the `AnimationPlayer` in code, and duplicates pack clips once with track paths prefixed `Body/`. Swap = set the export or call `set_skin()` at runtime (the showcase driver proves it).
- Rationale: per-agent skins in one scene need no prefab variants and no descendant override blocks on instances.

## Findings (headless dumps)

- `Knight.glb`: `Knight > Rig_Medium > Skeleton3D` (23 bones) + meshes, no `AnimationPlayer`.
- `Rig_Medium_General.glb`: same 23 bone NAMES (different order, irrelevant) + `AnimationPlayer` with `Idle_A/B, PickUp, Interact, Use_Item, Throw, Hit_A/B, Death_*`.
- `Rig_Medium_MovementBasic.glb`: `Walking_A/B/C, Running_A/B, Jump_*`.
- Tracks address `Rig_Medium/Skeleton3D:<bone_name>` — bone-name based, so they resolve on any Adventurers character that keeps its internal layout.
- Knight is ~2.54 m tall; skin scaled 0.6 (~1.53 m). Physics untouched (collision r=0.5/h=1.0, flat-quad navmesh).

## Interface

- `examples/source_3d/assets/agent/agent_skin_3d.gd`: lives on the skin-scene root (which instances one character `.glb`). Builds a shared static clip cache from the two packs once, fills an `AnimationPlayer` child, polls `entity` XZ velocity (`walk_threshold` → walk/idle). Exports: `entity`, `idle_animation`, `walk_animation`, `walk_threshold`, `blend_time`.
- `examples/source_3d/skins/knight_3d.tscn`: thin art scene — root instances `Knight.glb` at 0.6 + script + empty `AnimationPlayer` child. Swapping skins = new 6-line scene instancing another Adventurers `.glb`. Same rig ⇒ same clips, same socket heights.
- `agent_3d.tscn`: blue `CSGBox3D` placeholder (+ dead mesh/material) replaced by `Skin` instance; `Label3D` y 1.0→1.75; `CarrySocket` y 1.6→1.95 (clears the 0.6 m spinning log). Socket stays at agent root, so existing carry tests keep passing.
- `test/unit/test_agent_skin_3d.gd`: knight meshes present, `Idle_A`/`Walking_A` retargeted, walk-when-moving / idle-when-still via direct velocity.

## Verify

- Struct check; skin + carry GUT tests; full suite vs HEAD failure set; `lint-style`, `format-godot`.

## Follow-ups (not this pass)

- Remaining 4 skins; `PickUp`/`Use_Item` one-shots wired to dig/cook actions; run clip at higher speeds; hand-hold socket on `handslot` bones; label/socket heights per skin if proportions ever diverge.
