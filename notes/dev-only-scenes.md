# Dev-Only Scenes (removal tracking)

Intermediate visual-aid scenes, deleted once the real scenes are set up. (The agent showcase was reclassified permanent — only the gallery remains.)

## kaykit_gallery

- `examples/source_3d/viewer/kaykit_gallery.tscn` — 146-instance grid of every vendored model.
- `scripts/generate_kaykit_gallery.py` — regenerates it.
- Nothing depends on it (no scripts, no tests). Deps point inward only (vendored `.glb`, shared by real prefabs — keep those).
- Remove: delete both files (dir `viewer/` goes away empty).
