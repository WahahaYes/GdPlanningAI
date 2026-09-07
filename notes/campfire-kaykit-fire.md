# Campfire KayKit + Fire Particles (working plan)

Date: 2026-09-07. Goal: replace CSG placeholder logs/glow in `examples/source_3d/prefabs/campfire_3d.tscn` with KayKit wood + real `GPUParticles3D` fire. No live editor session, so text-edit `.tscn` + headless verify.

## Asset choice

- `Wood_Log_B` (`KayKit_ResourceBits/Assets/gltf/Wood_Log_B.gltf`, `uid://bi2ovcpk86cvh`): clean single log, 0.55 dia x 1.35 long along Z, centered. Fits inside the 0.8-radius collision (half-length 0.675).
- Rejected: `Wood_Log_A` (gnarlier, 0.72 wide), `Wood_Log_Stack` (1.68x1.5x1.5m — wood-pile scale, not campfire), `Stone_Chunks_Small` (fire-ring candidate, deferred).
- Layout: 3x `Wood_Log_B` flat star at y=0.28, yaws 0/60/120 deg (mirrors old Log1/Log2 pair, plus one).

## Prefab shape (new)

- Root `Campfire` keeps scripts/bindings, except `fire_primitive = NodePath("./Fire")` (was `./FireGlow`).
- `CollisionShape3D`, `Label3D` (billboard=1, no fixed_size), `GdPAI*` nodes unchanged.
- Delete: `FireGlow` CSGSphere, `Log1`/`Log2` CSGCylinders (+ their two `StandardMaterial3D`s).
- Add: `Log1..3` instances of `Wood_Log_B.gltf`; `Fire` (Node3D y=0.5) holding `FireParticles` (32, 0.8s), `SmokeParticles` (12, 1.6s), small `FireGlow` ember core (r=0.22, unshaded orange); `OmniLight3D` raised to y=1.0.
- Particle serialization copied from headless-generated reference (`/tmp/opencode/fire_ref.tscn`): `ParticleProcessMaterial` sphere emission, up velocity + gravity, `GradientTexture1D` color ramp, `QuadMesh` + unshaded particle-billboard `StandardMaterial3D`.
- `campfire_runtime.gd` needs no change: `_update_fire_visuals` scales any `Node3D`, so scaling `Fire` shrinks flames + ember together.

## Verify

- `godot --headless --path . --import` clean; prefab + `campfire_3d.tscn` instantiate headless with `Fire` path + particle counts asserted.
- `make test-godot` (campfire smoke is 2D-only, should be unaffected), `make lint-style`, `make format-godot`.
- Visual acceptance (orbit/mouse, flame look, log scale) needs a human with the editor open; MCP `particle_manage apply_preset fire` tune-up deferred to a live session.

## Follow-ups (not this pass)

- Flicker/light-energy coupling in `campfire_runtime.gd`; fuel-scaled particle `amount`.
- `wood_pile_3d.tscn` \<- `Wood_Log_Stack`; stone fire-ring; `Stone_Chunks_Small` navmesh carve check.
