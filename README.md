![GdPlanningAI banner](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/gdpai_banner.png)

# GdPlanningAI

GdPlanningAI (shortened as **GdPAI**) is an agent planning addon for Godot that allows you to build sophisticated AI agents for your game world.  These agents are able to reason in real-time and plan actions based on their own attributes and nearby interactable objects.

![GIF of the multi_agent_demo.tscn scene running](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/2d_demo.gif)

This framework is originally based on Goal Oriented Action Planning (GOAP), a planning system developed by Jeff Orkin in the early 2000's.  GOAP has been used in many games since; some popular titles using GOAP systems include F.E.A.R., Fallout 3, and Alien Isolation.  This framework started as a reimplementation of GOAP.  I noticed some areas for improvement and expanded the planning logics and place more emphasis on interactable objects.

The original motivation and "making of" process is covered here:

[![Link to a "making of" devlog video](https://img.youtube.com/vi/cm5Jxo31plw/0.jpg)](https://www.youtube.com/watch?v=cm5Jxo31plw)


### Installation

This repo is set up to function as a git submodule.  If you are not familiar with using submodules to maintain Godot plugins, they help with disentangling this project's updates with your own.  This project can be added to your project directory and used without being tracked or overwritten by your own version control.  

To install this addon as a git submodule, navigate to the top of your Godot project and run:

```
git submodule add https://github.com/WahahaYes/GdPlanningAI.git addons/GdPlanningAI
```

Then, inside the project editor, go to **Project -> Project Settings -> Plugins** and enable the GdPlanningAI addon.

Release versions are available on the Godot asset library [https://godotengine.org/asset-library/asset](https://godotengine.org/asset-library/asset).  **NOTE: The Godot Asset Library tries to import this addon at the top folder of your project because I have set it up to be better as a git submodule.  If installing through the asset library, make sure to click *Change Install Folder* and put the addon in `addons/GdPlanningAI`.**

**Script Templates**

A number of useful templates are included in the `script_templates` folder.  They guide usage when subclassing `Action`, `Goal`, `GdPAIObjectData`, etc.  The addon **does not automatically copy these over** at the moment, but it is highly recommended to use these templates.  If you'd like to take advantage of these, please copy the folders inside `addons/GdPlanningAI/script_templates` to your project's templates folder (by default this is `res://script_templates`).

## Usage

In this framework, agents form chains of actions at runtime rather than relying on premade state change conditions or behavior trees.  This can greatly reduce the amount of developer overhead when creating AI behaviors, but it is a more complex / less intuitive system.

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

### Demos

Demos with sample actions and objects are located in `GdPlanningAI/examples`.  Currently, there is a simple setup consisting of food objects and fruit trees.  The agents' main goal is to satisfy hunger.  Eating fruit will grant hunger, and shaking fruit trees will spawn fruit.  There is a delay interval before trees can be shaken again.  When the agent isn't hungry or there isn't food around, a wandering goal takes priority and the agent explores by moving in a random direction.  `examples/2D/single_agent_demo.tscn` shows a single agent and `examples/2D/multi_agent_demo.tscn` has two agents competing for the available food.  As an exercise, consider adding new goals and actions to this starting point!

![GIF of the multi_agent_demo.tscn scene running](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/2d_demo.gif)

A set of demos showcases the multithreading feature.  By multithreading agents in a complex scene, we see a 2x speedup! (on a laptop with an integrated GPU).  

![GIF of the multitheading_stress_test.tscn scene running](https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/multithread_demo.gif)

### License

GdPlanningAI, Copyright 2025 Ethan Wilson

This work is licensed under the Apache License, Version 2.0.  The license file can be viewed at [LICENSE.txt](LICENSE.txt) and at [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0).

**Demo assets**

The 2D demo assets belong to the Tiny Swords asset pack by Pixel Frog.  Link to the project page here: [https://pixelfrog-assets.itch.io/tiny-swords](https://pixelfrog-assets.itch.io/tiny-swords).

### TODOs

The framework is stable for creating planning agents but is still in an early phase of development.  I plan to make additions as I work on my game projects, and **I am open to feedback or contributions from the community!**  Please raise issues on the Github to discuss any bugs or requested features, and feel free to fork the repo and make pull requests with any additions.

Here is a running list of todo items *(if anyone wants to claim one, like logo or sprite artwork, please let me know!)*:

- Making a true project logo!  I quickly threw something together, but welcome a more professional looking logo.
- Making icons for the custom nodes that have been introduced.  Not that important for functionality, but they'd look nice!
- Creating a visual debugger similar to Beehave or LimboAI's debuggers for behavior trees.
- More varied and complex demo scenes.  Because the current demo uses `SpatialActions`, which bundle movement in with eating, the actual planning is quite simple, and most plans consist of a single action.
- Tutorial video.

### FAQs

Ask me questions!  If broadly relevant I'll add the Q&A here!
