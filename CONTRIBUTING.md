# Contributing to GdPlanningAI

## Getting Started

1. Fork the repository.
2. Create a feature branch.
3. Follow the style guidelines in `STYLE_GUIDELINES.md`.
4. Run the style checker before submitting.
5. Submit a pull request.

## Implementation Patterns

### Class Implementation
- Call `super()` in overridden `_init()` methods.
- Use script templates in `/script_templates/` for consistent implementation.
- Override required methods based on parent class.

### Common Method Overrides
The main classes to extend for functionality are:
- **Action**: Base class for agent actions.
- **Goal**: Base class for agent goals.
- **GdPAIObjectData**: Base class for interactable objects.
- **GdPAIBehaviorConfig**: Base class for behavior configurations.
- **PropertyUpdater**: Base class for property updates over time.

Use the script templates in `/script_templates/` for guidance on extending these classes.

## Pitfalls to Avoid

### Planning Simulation
- **Scene Tree Modifications**: Don't modify scene tree during planning simulation.
- **Super() Calls**: Always call `super()` in overridden `_init()` methods.
- **Precondition Validation**: Ensure all preconditions are properly validated.
- **Navigation Compatibility**: Test both 2D and 3D navigation compatibility.

## Style Validation

All code must follow the style guidelines outlined in `STYLE_GUIDELINES.md`. Run the automated style checker before submitting:

```bash
python tests/style_check.py
```

## Testing

- Test your changes with both 2D and 3D navigation when applicable.
- Use the provided script templates when creating new classes.
- Ensure all new code includes proper documentation.
- Verify that existing functionality is not broken.

## Submitting Changes

1. Ensure all code passes the style checker.
2. Test your changes thoroughly.
3. Update documentation if needed.
4. Submit a pull request with a clear description.
5. Respond to feedback and make requested changes.

## Project Structure

- `/addons/GdPlanningAI/`: Main addon code.
- `/script_templates/`: Templates for consistent class creation.
- `/examples/`: Example implementations and demo scenes.
- `/tests/`: Style checking and test utilities.

Thank you for contributing to GdPlanningAI!
