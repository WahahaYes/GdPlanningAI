![GdPlanningAI banner](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/gdpai_banner.png)

# Table of Contents
1. [Intro](#gdplanningai)
2. [Installation](#installation)
3. [Usage](#usage)
4. [Demos](#demos)
5. [License](#license)
6. [Frequently Asked Questions (FAQ)](#faq)
7. [TODOs](#todos)


# GdPlanningAI

GdPlanningAI (shortened as **GdPAI**) is an agent planning addon for Godot that allows you to build sophisticated AI agents for your game world.  These agents are able to reason in real-time and plan actions based on their own attributes and nearby interactable objects.

![GIF of the multi_agent_demo.tscn scene running](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/2d_demo.gif)

This framework is originally based on Goal Oriented Action Planning (GOAP), a planning system developed by Jeff Orkin in the early 2000's.  GOAP has been used in many games since; some popular titles using GOAP systems include F.E.A.R., Fallout 3, and Alien Isolation.  This framework started as a reimplementation of GOAP.  I noticed some areas for improvement and expanded the planning logics and place more emphasis on interactable objects.

The original motivation and "making of" process is covered here:

[![Link to a "making of" devlog video](https://img.youtube.com/vi/cm5Jxo31plw/0.jpg)](https://www.youtube.com/watch?v=cm5Jxo31plw)


### Installation

This repo is structured as a Godot addon.  This makes it straightforward to install via Godot's asset library.  The project is configured to copy `addons/GdPlanningAI` and the related `script_templates` folder to your project when you install the addon.

Release versions are available on the Godot asset library [https://godotengine.org/asset-library/asset](https://godotengine.org/asset-library/asset).

**Script Templates**

A number of useful templates are included in the `script_templates` folder.  They guide usage when subclassing `Action`, `Goal`, `GdPAIObjectData`, etc.

### Usage

In this framework, agents form chains of actions at runtime rather than relying on premade state change conditions or behavior trees.  This can greatly reduce the amount of developer overhead when creating AI behaviors, but it is a more complex / less intuitive system.

**Behavior System Architecture**

The framework now uses a modular behavior system that allows you to semantically group actions, goals, and properties. This system consists of:

- **GdPAIBehaviorConfig**: Base class for behavior configurations that group related goals, actions, and property updaters.
- **GdPAIAgentConfig**: Configuration resource that defines planning strategies and combines multiple behavior configs.
- **PropertyUpdater**: Components that modify agent blackboard properties over time (e.g., hunger decay, stamina regeneration).

This approach allows you to create reusable behavior modules (like "HungerBehavior" or "WanderBehavior") and combine them in different ways for different agent types.

**GdPAIAgent**

In this framework, each `GdPAIAgent` maintains two `GdPAIBlackboard` instances storing relevant information about their self and the broader world state.  An agent's own blackboard is used to maintain their internal attributes.  Possible attributes include *health*, *hunger*, *thirst*, *inventory*, etc.  The world state maintains common information, like *time of day* and information about interactable objects in-world (see `GdPAIObjectData` description below).

**Goal**

Agents are driven by `Goals`.  An agent will balance however many goals it is assigned it tries to maintain based on priority.  Given the agent and world states, a reward function is computed for each goal.  When planning, the agent pursues the most rewarding goal that is currently achieveable.  When designing goals, it is important to create dynamic reward functions so that the agent prioritizes different goals based on its needs (such as making a `hunger_goal` reward equal to `100 - current_hunger`, adding more priority the hungrier the agent gets).

![Goal planning diagram](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/goal_planning_diagram.png)

**Plan**

When an agent attempts to form a `Plan`, it essentially takes a snapshot of the current environment and simulates what would occur if various actions were taken.  The simulation creates temporary copies of all relevant action, blackboard, worldstate, and object data.  The copies exist outside of the scene graph, so here the planning agent is free to experiment and manipulate data attributes to test out various action sequences.

The below image gives a simple visual example for a planning sequence.  The agent's goal is to reduce hunger, which is ultimately resolved by eating food.  A prerequisite to eat food is to pick up the food, so the agent must first move towards the food.  

![A visual example of an agent's planning sequence](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/planning_sequence.png)

Note that planning actually occurs in reverse based on whether actions are viable for satisfying the plan or following actions.  This constrains the agent's exploration to only consider efficient, relevant actions.  The alternative would be a breadth-first search over a potentially huge state space.  In the above example, the agent first determined that the food on the map could decrease its hunger.  Then, the *pseudo-goal* became to determine how that food could be eaten (-> by going towards it).

**Action**

Plans are formed by chaining `Actions`.  After planning, these function similarly to leaf nodes of behavior trees, in that they *do* concrete actions.  For planning, actions have an additional set of `Preconditions` which are used to determine valid actions and pathfinding chains of actions.

An action has its preconditions organized via `Action.get_validity_checks()` and `Action.get_preconditions()`.  Validity checks are hard requirements which need to be true in order for the action to be considered at all during planning.  An example validity check is that the agent's blackboard contains a *hunger* attribute for a `eat_food` action.  

**Precondition**

Preconditions in `Action.get_preconditions()` are dynamic and necessary for the planning logic.  These are conditions that may not be true *yet*, but could be satisfied by other actions earlier on in a plan.  In the earlier example, a precondition for an `eat_food` action could be that the agent is holding food.  A `pickup_object` action satisfies this but has its own `object_nearby` precondition.  The `goto` action satisfies this and has no preconditions of its own (maybe `goto` had a validity check that the object in question was on the navmesh, which returned true).  By chaining preconditions together, the agent determined the chain of `goto` -> `pickup_object` -> `eat_food` as a valid solution.

The `Precondition` class evaluates a lambda function `eval_func(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard)`.  Please check the implemented preconditions in the example setup to get an understanding of how they can be written.  There are also a number of static functions in `Precondition` for common conditions.  **If you find yourself commonly creating preconditions of a certain format, please suggest an inclusion to the Precondition class or make a pull request!**

**GdPAIObjectData**

The final major component of this framework, and the most novel improvement over GOAP, is the inclusion of `GdPAIObjectData`.  This framework introduces an object-oriented approach where interactable objects broadcast the actions they provide.  Each subclass of `GdPAIObjectData` may broadcast its own action and functionality, and the composition of multiple of these under a single object results in an object that's usable in multiple ways.  During simulation, copies of this object data are moved outside the scene tree entirely so that it can be manipulated and simulated by the agent.  This enables much greater simulation potential than GOAP's dictionary-based simulation.

In addition to an agent's self-actions, which are not dependent on external factors (for example, maybe an agent has the action to rest to regain stamina), these `GdPAIObjectData` broadcast their relevent actions.  A `banana` object may broadcast the `eat_food` action.  The relevant subclass of `GdPAIObjectData` contains a `hunger_restored` attribute that the `eat_food` action references.  Through a validity check, the `eat_food` action ensures that agents have a `hunger` property, to prevent unnecessary computations for agents that don't become hungry.

The templates in `script_templates` and the demo in `examples/..` are verbosely commented to help with initial understanding of the framework.  Using the script templates is highly recommended when creating your own actions, goals, and object data classes.

**SpatialAction**

To streamline object interactions, the `SpatialAction` class bundles agent movement to an interactable object with a concrete action.  This is a helper subclass which aims to overcome an issue brought up with the original GOAP implementation - without careful design, actions which are **strongly coupled** might explode the planning complexity.  In GOAP, the issue they ran into was with `readying` and `firing` a weapon.  A weapon **always has to be ready before it can be fired**, so having these as separate actions greatly expanded the search space.  In prototyping, I noticed the same for `goto` and `object interactions`.  The agent must be near the object first, but with the option to *simulate movement anywhere during planning*, it became very slow for the agent to determine where it should be.  `SpatialAction` handles the logic for movement, then the subclass's implementation kicks in when the agent arrives at the object.

![Illustration of Action inheritence.  Spatial Actions are a subclass of action related to object interaction.](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/spatial_actions_hierarchy.png)

The `SpatialAction` class adapts to 2D or 3D depending on the location node specified on the object's `GdPAILocationData`.  `SpatialAction` has its own `script_template` that is expanded for this behavior; it is highly recommended to use the template.

### Agent Configuration

The new configuration system makes it easy to set up agents without writing code. Here's how to configure an agent:

1. **Create Behavior Configurations**: Extend `GdPAIBehaviorConfig` to create semantic behavior modules. For example:
   ```gdscript
   # HungerBehaviorConfig.gd
   class_name HungerBehaviorConfig
   extends GdPAIBehaviorConfig
   
   @export var hunger_decay: float = 5.0
   @export var initial_hunger: float = 100.0
   
   func _self_init() -> void:
       super()
       goals.append(SampleHungerGoal.new())
       property_updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
   ```

2. **Create Agent Configuration**: Create a `GdPAIAgentConfig` resource and assign behavior configs:
   - Choose planning strategy (continuous, interval-based, on-demand)
   - Configure multithreading settings if needed
   - Add multiple behavior configs to create complex agent personalities
   - Set up the blackboard plan with required properties

3. **Apply to Agent**: Assign the agent config to your `GdPAIAgent` node in the inspector

The system supports multiple planning strategies:
- **CONTINUOUS**: Plan every frame (default behavior).
- **ON_INTERVAL**: Plan at fixed time intervals (better for performance).
- **ON_DEMAND**: Plan only when explicitly requested.
- **ON_INTERVAL_FORCED**: Force planning at intervals, even if plan is active.

This modular approach allows you to:
- Mix and match behaviors (e.g., HungerBehavior + WanderBehavior + CombatBehavior).
- Create different agent types from the same building blocks.
- Easily tweak behavior parameters through the inspector.
- Share behavior configurations between different agent types.

### Debugging

There's now a visual debugger!  Located as its own debugger tab in the debugger window, it shows the plan graph and allows you to step through the plan to see which actions are a part of the current plan, their current status, and the traversal throughout the tree.  

![Illustration of the debugger tab](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/debugger_screenshot.png)

The debugger still lacks some useful features, like listing preconditions or the ability to step through the plan and see the agent's blackboard state at each step.  But this initial implementation should greatly help to understand the agents' behavior and the planning process.

![Illustration of the debugger tab](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/debugger_screenshot2.png)

### Demos

Demos with sample actions and objects are located in `examples/`.  You can download the whole project to play around with the demos.  The examples now demonstrate the new behavior system:

**Behavior System Examples**
- `examples/hunger/`: Complete hunger system with goals, actions, and property updaters.
- `examples/wander/`: Wandering behavior with configurable distance parameters.
- `examples/fruit_tree/`: Object interaction example with shakeable fruit trees.

**Demo Scenes**
- `examples/2D/single_agent_demo.tscn`: Shows a single agent using the behavior system.
- `examples/2D/multi_agent_demo.tscn`: Multiple agents competing for resources using different configurations.
- `examples/2D/multithread_single_agent.tscn`: Demonstrates multithreaded planning.
- `examples/2D/singlethreading_stress_test.tscn`: Performance testing with many single-threaded agents.
- `examples/2D/multithreading_stress_test.tscn`: Performance testing with many multithreaded agents.

The hunger system demonstrates how agents balance multiple goals - when hungry, they prioritize finding food, but when satisfied, they wander to explore.  The fruit tree example shows object interactions where agents can shake trees to spawn fruit, then eat the fruit to satisfy hunger.  As an exercise, consider creating new behavior configurations (like a combat behavior) and combining them with existing ones!

![GIF of the multi_agent_demo.tscn scene running](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/2d_demo.gif)

A set of demos showcases the multithreading feature.  By multithreading agents in a complex scene, we see a 2x speedup! (on a laptop with an integrated GPU).  

![GIF of the multitheading_stress_test.tscn scene running](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/multithread_demo.gif)

### License

GdPlanningAI, Copyright 2025 Ethan Wilson

This work is licensed under the Apache License, Version 2.0.  The license file can be viewed at [LICENSE.txt](LICENSE.txt) and at [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0).

**Demo assets**

The 2D demo assets belong to the Tiny Swords asset pack by Pixel Frog.  Link to the project page here: [https://pixelfrog-assets.itch.io/tiny-swords](https://pixelfrog-assets.itch.io/tiny-swords).

### FAQ

<details>
<summary><b>What is the difference between GdPlanningAI and behavior tree frameworks (like Beehave or LimboAI)?</b></summary>

These are all structured frameworks to develop agents / NPCS / enemies inside a game world.  Behavior trees have defined transitions to enable agent actions based on conditionals, but they require a ton of developer oversight (and design hours) as they become more complex.  Planning systems like GOAP and GdPlanningAI are more dynamic and can lead to emergent behaviors.  Even if the developer created every possible action, there may be combinations they didn't anticipate.  Because of this, planning systems can fit better for projects that have large numbers of possible interactions.

Below is a quote from a [blog post](https://zhuanlan.zhihu.com/p/110419210) with a good breakdown of the differences:

> BTs, roughly speaking, are a fancy way to encode complex sequences of rules. A Bt acts as a a sequence of programming statements (e.g., “if … then … else …") and basic loops. It takes as input the current state of the world and additional data (the blackboard) and return an action (or sequence of actions). For instances, rules can be like: “if your life is less than 40% then run away”, or “if do not have a weapon, go to the closest weapon”, and so on. If you want more info on BTs, there is this super-old introduction I did. The main point here is that you are writing all this rules. BTs are “reactive” in the sense that they “react” to the state of the world. There is no search, no thought about the future outcome of specific actions. It is the developer's job to specify which action is right in a certain situation. GOAP (and other plan-based AI technique), instead, works in a different way. You give to the character a goal (expressed as a desired state of the world) and a set of actions (the things that the character can do) and then you say to the character “now find your own rules”. There is no predefined sequence of actions in GOAP. Every time you run the algorithm, depending on the situation, it generates a different sequence of actions. Now, on paper, this is awesome. Why we are still writing all the sequence of actions and rules by hand! Unfortunately, we pay such power with three main drawbacks:
> 1. Much higher implementation complexity. BTs are quite easy to understand and to implement. Moreover, BTs are already built-in into a lot of game engines! GOAP, on the other hand, is not as simple. It harder to implement and it is harder to debug.
> 2. In general, plan-based techniques are computationally more expensive than BTs. Implementing GOAP in a way that is good enough for real-time games requires fine-tuning and a good design for the “state representation” and the set of possible actions (and we came back to point 1).
> 3. By not writing the rules of AI by ourselves, we lose control on the AI. If we say that the character goal is to kill the player, we may have some situation in which the solution to this goal is too effective or, in general, not fun. Because fun is the goal, we need to change this, but we can only act on the goal, not the way the character reaches the goal. For another example, if the character starts doing something strange it will be harder to understand “why”. In BTs, we can follow the tree and find the problem. In GOAP, this is much harder.
</details>


### TODOs

The framework is stable for creating planning agents but is still in an early phase of development.  I plan to make additions as I work on my game projects, and **I am open to feedback or contributions from the community!**  Please raise issues on the Github to discuss any bugs or requested features, and feel free to fork the repo and make pull requests with any additions.

Here is a running list of todo items *(if anyone wants to claim one, like logo or sprite artwork, please let me know!)*:

- Making a true project logo!  I quickly threw something together, but welcome a more professional looking logo.
- Making icons for the custom nodes that have been introduced.  Not that important for functionality, but they'd look nice!
- More varied and complex demo scenes.  Because the current demo uses `SpatialActions`, which bundle movement in with eating, the actual planning is quite simple, and most plans consist of a single action.
- Increased number of baseline action templates (like `SpatialAction`, extendable starting points that handle the common functionality of many actions).
- Tutorial video.
- Extending configuration for agents (such as planning strategy (continuous, on interval, etc.), how the world state is sourced, etc.).