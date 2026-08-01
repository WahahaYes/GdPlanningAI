![GdPlanningAI banner](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/gdpai_banner.png)

# Table of Contents

1. [Intro](#gdplanningai)
1. [Installation](#installation)
1. [Quick Start](#quick-start)
1. [Debugging](#debugging)
1. [Documentation](#documentation)
1. [License](#license)
1. [Frequently Asked Questions (FAQ)](#faq)
1. [TODOs](#todos)

# GdPlanningAI

GdPlanningAI (shortened as **GdPAI**) is a GOAP-based agent planning addon for Godot 4. Agents reason in real time and plan action chains from their own attributes and nearby interactable objects, instead of following hand-authored condition transitions. This can greatly reduce developer overhead when creating AI behaviors, but it is a more complex, less intuitive system than a behavior tree.

![GIF of the multi_agent_demo.tscn scene running](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/2d_demo.gif)

The framework started as a reimplementation of Goal Oriented Action Planning (GOAP), the planning system developed by Jeff Orkin and used in games like F.E.A.R., Fallout 3, and Alien Isolation. It was then expanded with a stronger emphasis on interactable objects and object-oriented simulation.

Under the hood the addon is two languages: a Rust planning engine with a GDScript API on top. See [docs/CODEBASE_OVERVIEW.md](docs/CODEBASE_OVERVIEW.md) for the repo map and [docs/ALGORITHM.md](docs/ALGORITHM.md) for how the search works.

The original motivation and "making of" process is covered here:

[![Link to a "making of" devlog video](https://img.youtube.com/vi/cm5Jxo31plw/0.jpg)](https://www.youtube.com/watch?v=cm5Jxo31plw)

### Installation

This repo is structured as a Godot addon, which makes it straightforward to install via Godot's asset library. Installing the addon copies `addons/GdPlanningAI` and the related `script_templates` folder into your project.

Release versions are available on the Godot asset library at [https://godotengine.org/asset-library/asset](https://godotengine.org/asset-library/asset).

**Script Templates**

The `script_templates` folder contains templates that guide you when subclassing `Action`, `Goal`, `GdPAIObjectData`, and the other core classes.

### Quick Start

The `examples/` folder contains demo scenes you can run right away. See `examples/README.md` for setup instructions and `docs/EXAMPLES.md` for step-by-step walkthroughs.

- **`hunger_basic_2d.tscn`**: single agent foraging, the simplest scene to start with.
- **`campfire_2d.tscn`**: multi-goal behavior: maintain the campfire and manage hunger.
- **`hunger_multi_agent_2d.tscn`**: multiple competing agents.
- **`hunger_stress_test_2d.tscn`**: performance test with many agents.
- **`campfire_3d.tscn`**: 3D version of the campfire demo.

The core extension points, in short:

- **`GdPAIAgent`**: the agent node. Each agent keeps two `GdPAIBlackboard` instances: one for its own attributes (hunger, health, inventory) and one for the world state (time of day, interactable objects).
- **`Goal`**: drives the agent. Each goal computes a reward function from the two blackboards, and planning pursues the most rewarding achievable goal. Use dynamic reward functions (for example a hunger goal with reward `100 - current_hunger`) so priorities shift with the agent's needs.
- **`Action` and `Precondition`**: actions form the plan and then execute it. Validity checks are hard requirements for an action to be considered during planning; preconditions are conditions that earlier actions in the chain can satisfy.
- **`GdPAIObjectData`**: subclasses broadcast the actions the world object provides. During simulation, copies of the object data are moved outside the scene tree so the planner can manipulate them freely.
- **`GdPAIBehaviorConfig` and `GdPAIAgentConfig`**: reusable behavior modules that group related goals, actions, and property updaters. The agent config picks a planning strategy (`CONTINUOUS`, `ON_INTERVAL`, `ON_DEMAND`, `ON_INTERVAL_FORCED`) and combines behavior configs to build different agent types.
- **`GoToAction`**: the generic navigation action provided by the agent. Its wildcard `at_target` provision satisfies any interaction action's location requirement, so the planner automatically chains `GoToAction` -> interaction action (for example `GoToAction` -> `PickupAction` -> `EatHeldFoodAction`).

Full authoring guide: [docs/AUTHORING_GUIDE.md](docs/AUTHORING_GUIDE.md). Algorithm details: [docs/ALGORITHM.md](docs/ALGORITHM.md).

### Debugging

The interactive debugger tab was removed as part of the Rust refactor and has not been reintroduced yet. Until it returns, the engine exposes the search tree as a text dump: `GdPAIPlanScheduler.get_debug_tree(agent)` returns a human-readable rendering of the agent's most recent planning job.

### Documentation

The relative links below resolve when reading this file on GitHub. On a local copy of the addon, the `docs/` links won't work; the latest version of the docs always lives at [github.com/WahahaYes/GdPlanningAI](https://github.com/WahahaYes/GdPlanningAI).

| Doc | Purpose | |-----|---------| | `docs/CODEBASE_OVERVIEW.md` | Repo map: addon anatomy, the two-language architecture (Rust engine + GDScript API), class hierarchy, build system, testing, and tooling. | | `docs/ALGORITHM.md` | How the planning search works: two-phase symbolic backward chaining plus forward simulation. | | `docs/PLANNER_PSEUDOCODE.md` | Terse algorithmic reference for the planning search. | | `docs/AUTHORING_GUIDE.md` | How to write actions, goals, preconditions, and object data. | | `docs/EXAMPLES.md` | Demo scenes: how to run them, behavior modules, object types, and plan-formation chains. | | `docs/DOCUMENTATION_GUIDELINES.md` | Code documentation style for this repo. |

### License

GdPlanningAI, Copyright 2025 Ethan Wilson

This work is licensed under the Apache License, Version 2.0. The license file can be viewed at [LICENSE.txt](LICENSE.txt) and at [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0).

**Demo Assets**

The 2D demo assets belong to the Tiny Swords asset pack by Pixel Frog: [https://pixelfrog-assets.itch.io/tiny-swords](https://pixelfrog-assets.itch.io/tiny-swords).

### FAQ

<details>
<summary><b>What is the difference between GdPlanningAI and behavior tree frameworks (like Beehave or LimboAI)?</b></summary>

These are all structured frameworks for developing agents, NPCs, and enemies inside a game world. Behavior trees give you defined transitions that enable agent actions based on conditionals, but they require a lot of developer oversight as they become more complex. Planning systems like GOAP and GdPlanningAI are more dynamic and can lead to emergent behaviors: even if the developer created every possible action, there may be combinations they did not anticipate. Because of this, planning systems can fit better for projects with large numbers of possible interactions.

Below is an abridged quote from a [blog post](https://zhuanlan.zhihu.com/p/110419210) with a good breakdown of the differences:

> Behavior trees are, roughly speaking, a way to encode complex sequences of rules. They react to the current world state, but there is no search and no thought about the future outcome of actions; it is the developer's job to specify which action is right in a given situation. Plan-based techniques like GOAP work differently: you give the character a goal and a set of actions, then tell it to find its own rules. There is no predefined sequence, and every run generates a different sequence of actions depending on the situation. That power comes with three main drawbacks: much higher implementation complexity, generally higher computational cost for real-time games, and less direct control over how the AI reaches its goals.

</details>

### TODOs

The framework is stable for creating planning agents but is still in an early phase of development. **I am open to feedback or contributions from the community.** Please raise issues on GitHub to discuss bugs or requested features, and feel free to fork the repo and submit pull requests.

- A true project logo (the current one is a quick placeholder; artwork welcome).
- Icons for the custom nodes.
- More varied and complex demo scenes.
- More baseline action templates.
- A tutorial video.
- Extending agent configuration (planning strategy, world-state sourcing, and more).
