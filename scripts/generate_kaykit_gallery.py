"""Generate a KayKit gallery viewer scene: one instance per glTF/GLB on a grid."""

import os
import re

ROOT = "/home/ethan/repos/GdPlanningAI"
VEND = "examples/source_3d/assets/vendored"
OUT = "examples/source_3d/viewer/kaykit_gallery.tscn"
COLS, SPACING = 16, 3.0


def main():
    models = []
    for dirpath, _, files in os.walk(os.path.join(ROOT, VEND)):
        for f in sorted(files):
            if f.endswith((".gltf", ".glb")):
                full = os.path.join(dirpath, f)
                models.append(os.path.relpath(full, ROOT).replace(os.sep, "/"))
    models.sort()
    assert models, "no models found"

    def nid(i):
        return f"{i}_g"

    lines = ["[gd_scene format=3]\n", "\n"]
    for i, m in enumerate(models, 1):
        lines.append(
            f'[ext_resource type="PackedScene" path="res://{m}" id="{nid(i)}"]\n'
        )
    lines += [
        "\n",
        '[sub_resource type="Environment" id="Environment_gallery"]\n',
        "background_mode = 2\n",
        "ambient_light_source = 3\n",
        "\n",
        '[node name="KayKitGallery" type="Node3D"]\n',
        "\n",
        '[node name="Ground" type="CSGBox3D" parent="."]\n',
        "position = Vector3(0, -0.25, 0)\n",
        "size = Vector3(60, 0.5, 40)\n",
        "\n",
        '[node name="Sun" type="DirectionalLight3D" parent="."]\n',
        "transform = Transform3D(0.707107, 0, -0.707107, 0, 1, 0, 0.707107, 0, 0.707107, 0, 10, 0)\n",
        "shadow_enabled = true\n",
        "\n",
        '[node name="WorldEnvironment" type="WorldEnvironment" parent="."]\n',
        'environment = SubResource("Environment_gallery")\n',
        "\n",
        '[node name="Camera3D" type="Camera3D" parent="."]\n',
        "transform = Transform3D(1, 0, 0, 0, 0.766044, 0.642788, 0, -0.642788, 0.766044, 0, 32, 38)\n",
        "current = true\n",
        "\n",
    ]
    x0 = -(COLS - 1) * SPACING / 2
    seen = set()
    for i, m in enumerate(models):
        stem = re.sub(r"\W+", "_", os.path.splitext(os.path.basename(m))[0]).strip("_")
        name, k = stem, 2
        while name in seen:
            name, k = f"{stem}_{k}", k + 1
        seen.add(name)
        col, row = i % COLS, i // COLS
        x = x0 + col * SPACING
        z = row * SPACING
        lines.append(
            f'[node name="{name}" parent="." instance=ExtResource("{nid(i + 1)}")]\n'
            f"position = Vector3({x:g}, 0, {z:g})\n\n"
        )
    os.makedirs(os.path.join(ROOT, os.path.dirname(OUT)), exist_ok=True)
    with open(os.path.join(ROOT, OUT), "w") as f:
        f.writelines(lines)
    rows = (len(models) + COLS - 1) // COLS
    print(f"wrote {OUT}: {len(models)} instances, {COLS}x{rows} grid")


if __name__ == "__main__":
    main()
