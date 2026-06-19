# Machine Learning & AI Integration Guide for akiuS

This document serves as a developer and AI agent reference guide for designing, training, and implementing machine learning (ML) models or search-based AI agents to play **akiuS**. It outlines the game's mechanics, problem space, relevant codebase architecture, and implementation roadmaps.

---

## 1. The Problem Space & Game Mechanics

**akiuS** is a 3D physics-based merge puzzle game (in the genre of *Suika Game* or *Watermelon Game*). The player aim-slides a launcher along a single horizontal axis and shoots spheres of various tiers onto a play table. 

### Key Mechanics
* **Aiming & Shooting**: The launcher slides along the X-axis within fixed horizontal limits. Spheres are shot downwards with an initial downward impulse.
* **Physics & Merging**: Spheres obey gravity, friction, and elasticity (simulated via `bevy_rapier3d`). When two spheres of the same tier collide, they merge into a single sphere of the next tier up (tier $T \rightarrow T+1$), conserving momentum and scaling up in volume/radius.
* **Active Orders**: Fulfilling the game's orders requires creating a sphere of a specific target tier [TRG] (e.g., Tier 6).
* **Loss Conditions**: 
  1. **Overflow**: Spheres stack too high and cross the launcher's horizontal threshold (checked after a grace period).
  2. **Table Spill**: Spheres fall off the sides of the play table (checked via height boundaries).
* **Scoring**: Merging spheres awards score points based on the resulting tier. Fulfilling orders awards order-completion bonuses.

### AI Challenges
1. **Continuous Action Space**: The aiming coordinate is continuous ($X \in [-X_{\text{limit}}, X_{\text{limit}}]$).
2. **Delayed Feedback**: The immediate action (launching a sphere) does not yield an instant score. The sphere must travel, bounce, settle, and collide. Multiple cascade merges can happen seconds after a drop.
3. **Complex Physics Simulation**: Collisions, friction, and stacking are continuous and non-linear. Small changes in aim can lead to wildly different settling patterns.
4. **State Accumulation**: Choices are constrained by the history of past drops. Piling too many small spheres at the bottom restricts space, leading to inevitable board overflows.

---

## 2. Codebase Architecture Summary

For an external AI agent or developer looking to integrate an ML environment, the relevant files in the `src/` directory are:

* **[src/game_state.rs](file:///Users/watcher/githere/akius/src/game_state.rs)**:
  Defines the core game structs, including `Score`, `DispenserQueue` (holds the current and next sphere in queue), `ActiveOrder` (holds the target tier), and the logic checking for board overflows, table spills, and order completions.
* **[src/physics.rs](file:///Users/watcher/githere/akius/src/physics.rs)**:
  Defines physics properties (mass, restitution, friction), sphere radius calculations (`radius = base_radius * (1.2 ^ tier)`), collision-merging triggers, momentum conservation math, and spawn behaviors.
* **[src/launcher.rs](file:///Users/watcher/githere/akius/src/launcher.rs)**:
  Defines the launcher boundaries, aiming constraints, cooldown timers, and initial shoot impulse vectors.
* **[src/core_math.rs](file:///Users/watcher/githere/akius/src/core_math.rs)**:
  Contains the probability weight tables for dispenser tier generation.

---

## 3. Designing a Gymnasium Environment

To train reinforcement learning models, the game needs to be wrapped in a standard Gym/Gymnasium interface (usually in Python).

### Observation Space (The State)
A clean state vector representing the board should include:
* **Launcher Queue**: Current sphere tier ($T_{\text{current}}$) and next sphere tier ($T_{\text{next}}$).
* **Target Objective**: Target order tier ($T_{\text{target}}$).
* **Active Spheres**: A variable-length list or fixed-size array of active spheres on the board, represented as features:
  $$\text{Sphere}_i = [x_i, y_i, z_i, \text{radius}_i, \text{tier}_i, v_{x,i}, v_{y,i}, v_{z,i}]$$
* **Visual Alternative (Voxel Grid)**: A discretized 2D/3D occupancy grid mapping the heights and tiers of the board.

### Action Space
* **Drop Location**: A continuous float $x \in [-1.0, 1.0]$, mapped to the launcher's horizontal travel bounds. 
* **Launch Trigger**: Optional discrete trigger (if aiming and dropping are decoupled). For a simpler turn-based setup, the action is simply: "Slide to $X$ and immediately drop."

### Reward Function Design
* **Positive Rewards**:
  * $+S$ (Score points gained from merging).
  * $+O$ (Fulfillment bonus when order target tier is met).
* **Negative Rewards**:
  * $-P_{\text{spill}}$ (Large penalty for triggering a table spill loss).
  * $-P_{\text{overflow}}$ (Large penalty for triggering an overflow loss).
  * $-H_{\text{max}}$ (A small step-wise penalty proportional to the height of the tallest sphere to encourage keeping the board low).

---

## 4. Implementation Strategies

### Strategy A: Monte Carlo Tree Search (MCTS)
Because the physics are deterministic, MCTS is highly effective and requires **no training time**.
1. **Branching**: For the current sphere, sample $N$ possible drop coordinates (e.g., 10 slots across the X-axis).
2. **Simulation**: Clone the current physics engine state. Step the physics engine forward $T$ seconds (e.g., 4–5 seconds, until all spheres settle).
3. **Evaluation**: Compute a heuristic value for each future state:
   $$\text{Heuristic} = \text{Score Gained} - \text{Max Height} - \text{Spill Risk} + \text{Potential Merge Clusters}$$
4. **Decide**: Choose the path with the highest score and execute it.

### Strategy B: Reinforcement Learning (RL)
To train a model to play tabula rasa:
1. **Headless Engine**: Build a headless version of `akius`. In `Cargo.toml`, Bevy's `WindowPlugin` and `RenderPlugin` can be disabled or bypassed for training.
2. **Python Bindings**: Use **PyO3** and **Maturin** to expose the Rust physics step loops and game state directly to Python.
3. **Algorithm**: Connect the Python wrapper to **Stable-Baselines3** and train using:
   * **DQN** (Deep Q-Networks) by discretizing the aiming axis.
   * **PPO** (Proximal Policy Optimization) for continuous aiming control.

### Strategy C: AlphaZero Hybrid
1. Train a Neural Network to act as a **Value Network** (predicting the expected future score of a board state) and a **Policy Network** (suggesting drop locations).
2. Use MCTS to look ahead, utilizing the Policy Network to narrow down search paths and the Value Network to evaluate leaf nodes instead of relying on hand-written heuristics.
