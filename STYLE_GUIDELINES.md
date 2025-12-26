# GdPlanningAI Style Guidelines

This document outlines the coding standards and style guidelines for contributors to the GdPlanningAI project.

## GDScript Formatting

### Basic Formatting
- Use tabs for indentation (Godot standard).
- Maximum 100 characters per line.
- Add explicit return types to all functions: `func my_function() -> void:`.
- Prefix unused parameters with underscores: `_param_name`.

### Function Signatures
- Functions with multiple parameters must span multiple lines.
- Function parameters should be aligned vertically when spanning multiple lines.
- All functions must declare explicit return types (even `-> void` for no return).

### Variable Declarations
- Use explicit variable typing with colon syntax: `var x: int = 1`.
- Do not use `var x := 1` syntax.
- All variable declarations must include explicit types.

## Documentation Standards

- Use `##` for class and member documentation.
- Use `#` for implementation comments.
- End comments with periods.
- Use `[br]` for line breaks in documentation strings.

## Naming Conventions

- Classes: PascalCase (`GdPAIAgent`, `SpatialAction`).
- Functions/variables: snake_case (`get_action_cost`, `blackboard`).
- Constants: UPPER_CASE (`MAX_RECURSION`).
- Private members: prefix with underscore (`_current_plan`).

## Style Validation

Run the automated style checker before submitting:

```bash
python tests/style_check.py
```

I recommend also using a godot formatter (I use the godot-format VSCode extension) for automated formatting for GDScript files on save.

### Common Style Violations

- Multi-parameter functions on single line.
- Functions without explicit return types.
- Lines over 100 characters.
- Missing explicit variable types.

All code must pass the style checker before being merged.
