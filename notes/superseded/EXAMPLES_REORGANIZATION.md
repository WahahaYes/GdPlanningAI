# GdPlanningAI — Examples Reorganization to Top-Level

## Goals

Move examples out of `addons/GdPlanningAI/examples/` into a top-level `examples/` folder. This allows users who install the addon to exclude the example suite, reducing distribution size and keeping addon installations clean.

**Benefits:**
1. **Cleaner addon distribution** — users only get core plugin files
2. **Reduced install size** — examples are opt-in, not bundled
3. **Clearer separation** — examples are demo projects, not part of the plugin
4. **Better for AssetLib** — addon submissions don't include demo assets

---

## Current Structure

```
c:\Godot\GdPlanningAI\
  addons\
    GdPlanningAI\
      bin\                    ← Rust GDExtension binaries
      examples\               ← TO BE MOVED
        shared\
          behaviors\
          objects\
        demo_2d\
          scenes\
          assets\
          configs\
        README.md
      rust\                   ← Rust source code
      scripts\                ← GDScript framework
        nodes\
        refcounteds\
        resources\
      gdpai_autoload.gd
      gdpai_utils.gd
      gdplanningai.gdextension
      plugin.cfg
      plugin.gd
      LICENSE.txt
      README.md
```

---

## Target Structure

```
c:\Godot\GdPlanningAI\
  addons\
    GdPlanningAI\
      bin\                    ← Rust GDExtension binaries
      rust\                   ← Rust source code
      scripts\                ← GDScript framework
        nodes\
        refcounteds\
        resources\
      gdpai_autoload.gd
      gdpai_utils.gd
      gdplanningai.gdextension
      plugin.cfg
      plugin.gd
      LICENSE.txt
      README.md

  examples\                   ← NEW TOP-LEVEL FOLDER
    shared\
      behaviors\
        hunger\
        wander\
        fire_maintenance\       ← NEW for campfire example
      objects\
        food\
        fruit_tree\
        wood_pile\              ← NEW
        campfire\               ← NEW
    demo_2d\
      scenes\
        single_agent_demo.tscn
        multi_agent_demo.tscn
        many_agents_demo.tscn
        campfire_demo.tscn      ← NEW
      assets\
        agent\
        world\
        ui\
      configs\
    demo_3d\                    ← NEW
      scenes\
        campfire_demo.tscn      ← NEW
      assets\
        agent\
        world\
    README.md
```

---

## Migration Steps

### Step 1: Create Top-Level examples/ Directory
```bash
mkdir c:\Godot\GdPlanningAI\examples
```

### Step 2: Move Examples Content
```bash
# Move entire examples folder contents
mv addons/GdPlanningAI/examples/* examples/
```

### Step 3: Update Path References

**Files that reference examples:**

1. **`examples/README.md`**
   - Update relative paths to scenes (no change needed - paths are relative within examples/)

2. **Scene files (`.tscn`)**
   - All resource paths starting with `res://addons/GdPlanningAI/examples/` 
   - Change to `res://examples/`
   - Affects: scripts, textures, configs, prefabs

3. **Script files**
   - Any `preload()` or `load()` calls with `res://addons/GdPlanningAI/examples/`
   - Change to `res://examples/`

4. **`addons/GdPlanningAI/README.md`**
   - Update example paths in documentation
   - "See examples in `examples/` folder at project root" instead of "examples/"

5. **Project root `README.md`**
   - Add section about examples folder
   - Document how to run examples

### Step 4: Update .gitignore (if needed)
```
# Don't ignore examples/ - it's part of the repo
```

### Step 5: Test All Examples
- [ ] Open `examples/demo_2d/scenes/single_agent_demo.tscn` - verify no missing resources
- [ ] Play scene - verify all scripts load
- [ ] Open `examples/demo_2d/scenes/multi_agent_demo.tscn` - verify
- [ ] Open `examples/demo_2d/scenes/many_agents_demo.tscn` - verify

---

## Files Requiring Path Updates

### Scene Files (`.tscn`)
All scenes in `examples/demo_2d/scenes/` will have internal paths like:
```
[ext_resource type="Script" path="res://addons/GdPlanningAI/examples/demo_2d/assets/agent/nav_controller.gd" id="1"]
```

These need to become:
```
[ext_resource type="Script" path="res://examples/demo_2d/assets/agent/nav_controller.gd" id="1"]
```

**Affected files:**
- `examples/demo_2d/scenes/single_agent_demo.tscn`
- `examples/demo_2d/scenes/multi_agent_demo.tscn`
- `examples/demo_2d/scenes/many_agents_demo.tscn`
- `examples/demo_2d/assets/agent/agent.tscn`
- `examples/demo_2d/assets/world/scenery.tscn`
- `examples/demo_2d/assets/world/huge_scenery.tscn`
- `examples/demo_2d/assets/world/banana.tscn`
- `examples/demo_2d/assets/world/banana_tree.tscn`

**Strategy:** Use find-and-replace in each `.tscn` file:
- Find: `res://addons/GdPlanningAI/examples/`
- Replace: `res://examples/`

### Resource Files (`.tres`)
- `examples/demo_2d/configs/agent_config.tres`
- `examples/shared/behaviors/hunger/hunger_behavior_config.tres`
- `examples/shared/behaviors/wander/wander_behavior_config.tres`

**Strategy:** Same find-and-replace as scenes.

### GDScript Files
Most scripts use relative class names (`class_name HungerGoal`) so no path updates needed.

**Check for explicit paths:**
```bash
grep -r "res://addons/GdPlanningAI/examples" examples/
```

If any found, update to `res://examples/`

---

## Documentation Updates

### Update `addons/GdPlanningAI/README.md`

Replace examples section with minimal reference:
```markdown
## Examples

Example scenes are available in the `examples/` folder at the project root. See `examples/README.md` for details.
```

### Update Project Root `README.md`

Add brief examples section:
```markdown
## Examples

The `examples/` folder contains demonstration scenes. See `examples/README.md` for setup instructions and detailed documentation.
```

### Update `examples/README.md`

Expand with comprehensive instructions and overview:

```markdown
# GdPlanningAI Examples

**Note:** These examples are located at the project root (`examples/`) to keep them separate from the core addon installation. When distributing or installing just the addon, this folder can be excluded.

---

## Setup

1. Open this project in Godot 4.x
2. Enable the GdPlanningAI addon if not already enabled:
   - Project → Project Settings → Plugins
   - Enable "GdPlanningAI"
3. Navigate to `examples/demo_2d/scenes/` or `examples/demo_3d/scenes/` in the FileSystem dock

---

## Running Examples

### 2D Demos

**Navigate to:** `examples/demo_2d/scenes/`

- **`single_agent_demo.tscn`** — Basic agent with hunger management
  - One agent foraging for food and shaking fruit trees
  - Good starting point to understand core concepts
  
- **`multi_agent_demo.tscn`** — Multiple agents competing for resources
  - Several agents with independent planning
  - Demonstrates resource contention and goal prioritization
  
- **`many_agents_demo.tscn`** — Performance stress test
  - Many agents (configurable spawner) 
  - Tests planning engine performance under load
  
- **`campfire_demo.tscn`** — Maintenance and proactive planning
  - Agents balance hunger vs campfire fuel maintenance
  - Demonstrates multi-step preparation and time-critical actions

**To run:** Open any `.tscn` file and press **Play Scene** (F6)

### 3D Demos

**Navigate to:** `examples/demo_3d/scenes/`

- **`campfire_demo.tscn`** — 3D version of campfire maintenance demo
  - Same behaviors as 2D version, different visual presentation
  - Demonstrates dimension-agnostic code design

**To run:** Open the `.tscn` file and press **Play Scene** (F6)

---
```

Then continue with existing folder layout and concept map sections...

---

## Script Template Updates

Script templates in `script_templates/` reference examples. Check if they use absolute paths:

```bash
grep -r "addons/GdPlanningAI/examples" script_templates/
```

If any exist, update to:
```
res://examples/
```

---

## Asset Library Considerations

When submitting to Godot Asset Library:

**addon/ folder only:**
```
addons/GdPlanningAI/
  (core files only)
```

**Full project with examples:**
```
addons/GdPlanningAI/
examples/
README.md
project.godot
```

Users can choose:
- **Minimal install:** Copy just `addons/GdPlanningAI/` to their project
- **With examples:** Clone full repo or download with examples included

---

## Automation Script

### Bash/PowerShell Script to Perform Migration

```powershell
# move_examples.ps1

$source = "addons\GdPlanningAI\examples"
$dest = "examples"

# Create destination
New-Item -ItemType Directory -Force -Path $dest

# Move contents
Move-Item -Path "$source\*" -Destination $dest -Force

# Remove old folder
Remove-Item -Path $source -Force

# Update paths in .tscn and .tres files
Get-ChildItem -Path $dest -Recurse -Include *.tscn,*.tres | ForEach-Object {
    $content = Get-Content $_.FullName -Raw
    $updated = $content -replace 'res://addons/GdPlanningAI/examples/', 'res://examples/'
    Set-Content -Path $_.FullName -Value $updated -NoNewline
}

Write-Host "Migration complete. Test examples in Godot to verify."
```

**Usage:**
```powershell
cd c:\Godot\GdPlanningAI
.\move_examples.ps1
```

---

## Testing Checklist

After migration:

- [ ] Open Godot project
- [ ] Verify no missing resource errors in FileSystem
- [ ] Open `examples/demo_2d/scenes/single_agent_demo.tscn`
- [ ] Check Output panel for path errors
- [ ] Play scene - verify agents behave correctly
- [ ] Repeat for `multi_agent_demo.tscn`
- [ ] Repeat for `many_agents_demo.tscn`
- [ ] Open example behavior configs in Inspector - verify scripts load
- [ ] Open example object prefabs - verify scripts load
- [ ] Git status - verify old examples/ removed, new examples/ added
- [ ] Commit changes

---

## Campfire Example Path Updates

When implementing the campfire example (per `CAMPFIRE_EXAMPLE_PLAN.md`), use new paths:

```
examples/
  shared/
    behaviors/
      fire_maintenance/
        fire_maintenance_goal.gd
        fire_fuel_updater.gd
        fire_maintenance_behavior_config.gd
    objects/
      wood_pile/
        wood_pile_object.gd
        pick_up_wood_action.gd
      campfire/
        campfire_object.gd
        add_fuel_action.gd
  demo_2d/
    scenes/
      campfire_demo.tscn
    assets/
      world/
        wood_pile.tscn
        campfire.tscn
  demo_3d/
    scenes/
      campfire_demo.tscn
    assets/
      world/
        wood_pile.tscn
        campfire.tscn
      agent/
        agent.tscn
```

All new files should use `res://examples/` paths, not `res://addons/GdPlanningAI/examples/`.

---

## Rollback Plan

If issues arise:

```powershell
# Revert (before committing)
git restore .

# OR manually:
Move-Item -Path examples\* -Destination addons\GdPlanningAI\examples -Force

# Then re-update paths back to old structure
Get-ChildItem -Path addons\GdPlanningAI\examples -Recurse -Include *.tscn,*.tres | ForEach-Object {
    $content = Get-Content $_.FullName -Raw
    $updated = $content -replace 'res://examples/', 'res://addons/GdPlanningAI/examples/'
    Set-Content -Path $_.FullName -Value $updated -NoNewline
}
```

---

## Estimated Time

- **Migration execution:** 15 minutes (script + manual verification)
- **Testing all scenes:** 15 minutes
- **Documentation updates:** 15 minutes

**Total:** ~45 minutes

---

## Related Documents to Update

After migration, update these planning documents with new paths:

- `CAMPFIRE_EXAMPLE_PLAN.md` - Update file structure section
- `EXAMPLES_PLAN.md` - Update proposed structure (already executed)
- Any other docs referencing `addons/GdPlanningAI/examples/`
