# Wood Pile KayKit + Wood Carry Visual (working plan)

Date: 2026-09-07. Status: DONE — 6/6 carry tests pass, failure set identical to HEAD, lint clean. No `campfire_3d.tscn` hookup yet.

## Asset choice

- Pile: `Wood_Log_Stack` (`uid://d2a0xkxpletpo`), 1.68 x 1.5 x 1.5 m, base at local y~0 — sits on the ground as-is. Replaces the `CSGBox3D` placeholder in `wood_pile_3d.tscn` (uid/wiring kept, collision resized to match, label raised above the stack).
- Held: `Wood_Log_B` (`uid://bi2ovcpk86cvh`) instanced at 0.45 scale (~0.6 m, overhead-readable next to the 0.25 m potatoes). Vendored glTF carries no GdPAI nodes, so direct instancing in the held scene is safe.
- No physics redesign: piles keep the existing pattern (bare `CollisionShape3D`, no body — same as campfire); navmesh stays one flat quad.

## Files

- `examples/source_3d/prefabs/wood_pile_3d.tscn`: `LogStack` instance + `BoxShape3D` 1.7/1.5/1.5 at y=0.75, `Label3D` at y=1.8.
- `examples/source_3d/prefabs/held_wood_3d.tscn` (new, visual-only): `HeldWood` root + scaled `Wood_Log_B`.
- `examples/source_3d/assets/agent/carry_visual_3d.gd`: `+WOOD_SCENE`, `"wood"` in the default map.
- `test/unit/test_carry_visual_3d.gd`: unmapped probe switches `"wood"` -> `"stone"`, new `test_wood_visual_appears`.

## Verify

- Python ext/sub-resource structural check; carry GUT tests; full suite diffed against HEAD failure set; `lint-style`, `format-godot`.
