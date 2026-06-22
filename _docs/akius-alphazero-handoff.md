# akiuS AlphaZero Agent — Design Handoff

Context: akiuS is Cyrus's Rust/Bevy/Rapier3D physics merge-puzzle game, built in Bevy 3D with bevy_rapier3d. This document captures a full design conversation for building an AlphaZero-style agent to play it, motivated partly by a concrete research question: **is the undocumented Tier 9 win condition actually achievable?**

This is a handoff for continuing the conversation in a fresh context window — it should be enough to pick up implementation planning without re-deriving any of the design.

---

## 1. The Game (as established across the conversation)

- **Table**: flat horizontal surface, X width 8.0 (−4.0 to 4.0), Z depth 14.0 (−2.0 to 12.0). Launcher sits at the open front edge (Z=12.0) and slides along X. Shooting sends spheres **forward** (decreasing Z), not downward — this is a "bowling alley," not a Suika-style gravity well.
- **Spheres**: Y-axis translation and all rotation are locked (`LockedAxes::TRANSLATION_LOCKED_Y` etc.). No floor friction (Y locked means no normal force). Lateral deceleration is **pure linear damping**, coefficient 0.8.
    - Radius: `R(T) = 0.5 × 1.21^(T-1)`. Diameters: T1=1.00 ... T8=3.80, T9=4.60.
    - Slide physics: `v(t) = v₀e^(−0.8t)`, max drift `s_max = 1.25v₀`. At 24-tick (0.4s) sampling, even high-velocity post-merge spheres (v₀=2.5) only travel ~0.86 units between samples — **guaranteed to sample at least once while any sphere crosses any point on the board**, since smallest diameter is 1.0. This validates 24-tick decision granularity.
- **Loss conditions**:
    - **Spill/overflow**: spheres pushed back toward the launcher until Z > 12.2, held >0.5s grace period. This is the _real_ pressure axis — the table fills lengthwise (low Z accumulates first, since spheres drift toward the back wall) and danger means spheres getting pushed back toward Z=12.0, not vertical stacking.
    - **Fallen**: sphere center Y < −0.2 (fell off a side edge).
- **Mandatory order system**: target tier drawn from {5,6,7,8}, dynamically weighted:
    - T5: flat 5%
    - T8: `(completed_orders × 10).min(40)` — 0% at start, ramps to 40% cap after 4 completions
    - T6: `60.saturating_sub(completed_orders × 10).max(15)`
    - T7: remainder (~35-40%)
    - **Fulfillment is involuntary and immediate**: fires the instant a matching-tier sphere exists (whether just merged or already on board when target rolls), locks it, despawns over 1.2s. No player choice — this matters a lot for reward design (see §4).
- **Launcher cooldown**: 0.4s (24 ticks) minimum between shots. Player _can_ shoot into active cascades (deliberate hail-Mary mechanic — knocking drifting/falling spheres back from the spill edge).
- **Win condition (undocumented, the research target)**: merge to **Tier 9**. Orders only go to T8, so T9 is a "secret" objective the designer (Cyrus) added speculatively and isn't sure is achievable.
- **Key emergent mechanic**: spheres drift toward the back wall (low Z) by default. Merge-and-despawn cascades at the back wall can leave surviving neighbor spheres **stranded mid-table** — this is the only way to get two T8s coexisting with clear space behind them, which is likely necessary for a T9 attempt (two T8s side by side = 7.6 of 8.0 width, almost no room for anything else in that Z-slice).

### The T9 feasibility analysis

- Geometrically possible: two T8s (diameter 3.80 each) fit side-by-side in 8.0 width with 0.2 units spare per side.
- Build tree in the worst case (no dispenser help) requires up to 256 base spheres feeding up through tiers — exponential, but dispenser tier variety shortens this drastically in practice.
- **The real constraint is the order system**, not geometry. Since fulfillment is involuntary, you cannot "protect" a T8 once it exists — your only lever is _not creating_ a lone T8 until you're ready to immediately pair it. Strategy implication: build both T7→T8 paths in parallel and execute both final merges in rapid succession to minimize the window (K) where exactly one T8 exists. Probability of surviving K order rolls without a T8 hit is roughly `≥0.6^K`(since T8 weight caps at 40%). Minimizing K is the core T9 strategy.
- Three possible research outcomes once trained: (1) agent reliably achieves T9 → confirmed possible, strategy discovered; (2) agent never achieves it despite heavy incentive and many attempts at the necessary board state → reasonably strong evidence of infeasibility under mandatory orders; (3) agent achieves it only when order RNG happens to cooperate → possible but luck-gated, interesting finding in itself.

---

## 2. Implementation Strategy Comparison (covered, for reference)

- **MCTS only**: no training required, exploits determinism, one-shot heuristic evaluation per simulated drop. Good first move / sanity check but heuristics must be hand-written and limited to short lookahead.
- **Pure RL (PPO/DQN)**: requires headless engine + PyO3/Maturin Python bindings. Reward shaping needed because of delayed/cascading feedback; risk of reward hacking.
- **AlphaZero hybrid (chosen direction)**: policy network narrows MCTS branching, value network replaces hand-written heuristics, MCTS-generated visit distributions become training targets for the policy network (the "bootstrapping" loop). Chosen specifically because Cyrus's own high score is far below a friend's — i.e., explicit motivation to _not_ hand-encode human strategic assumptions. Compared favorably against the "Getting/Losing the World Record in Hatetris" ML efforts as a mental model (same genre of "let the system prove what's achievable" problem).
    - **DAG vs tree MCTS** (a Hatetris technique) does **not** transfer — Hatetris board states are discrete/finite so duplicate-state detection saves real work; akiuS states include continuous positions and velocities, so true duplicate states essentially never occur. Skip this.
    - **NNUE** (incremental-update chess/Tetris network architecture) also does **not** transfer — its efficiency relies on sparse, discrete state diffs between evaluations; akiuS state is dense and continuously changing every tick due to physics. The conv-net design below is the right fit instead.

---

## 3. Observation Space (final design)

**Spatial component** — 16×28×3 grid (0.5-unit cells across X×Z), fed through conv layers:

- Channel 0: sphere tier at cell (0 = empty)
- Channel 1: **signed Z-velocity** (positive = moving toward launcher/danger direction) — this is the corrected "danger axis" channel, originally mis-designed around Y/height before the table orientation was clarified
- Channel 2: speed magnitude

Suggested grid orientation: row 0 = Z=0 (back wall), row 27 = Z=12.0 (launcher edge), so increasing row index = increasing danger, for convolutional consistency.

**Scalar component** — 15 values, concatenated after conv layers:

```
current_sphere_tier, next_sphere_tier          (launcher queue)
order_target_tier, completed_orders            (order state)
count_t5, count_t6, count_t7, count_t8         (order exposure risk)
mean_kinetic_energy, max_sphere_height          (board summary — NOTE: "height" 
                                                  here was later corrected to mean 
                                                  Z-proximity-to-launcher, not Y)
in_flight_tier, in_flight_x, in_flight_z, in_flight_vy   (mid-flight sphere, 
                                                            relevant for hail-Mary shots)
ticks_since_last_shot                           (timing context)
```

Velocity channels are **load-bearing, not optional** — the hail-Mary mechanic (shooting into an active cascade to deflect a sphere headed toward the spill edge) requires the agent to reason about directional momentum, not just static positions.

---

## 4. Action Space (final design)

- **32 discrete shoot positions** across X ∈ [−4.0, 4.0] (0.25 unit resolution — finer than any sphere diameter at any tier).
- **3 wait actions (micro-waits)**:
  * **Wait 6 ticks** (0.1s)
  * **Wait 12 ticks** (0.2s)
  * **Wait 24 ticks** (0.4s) — standard default wait
- **Why micro-waits are used**: proactive design decision to provide sub-0.4s timing control. While 24-tick granularity is fine for general board observation, timing resolution of 0.4s is too coarse for precise "hail-Mary" deflection shots on moving spheres (e.g., a target moving at $1.5\text{ m/s}$ shifts $0.3\text{ units}$ in $0.2\text{s}$, which is significant compared to sphere radii). Introducing these in advance avoids neural network shape mismatch and having to discard checkpoints/restart training later.
- **Critical correction during design**: since nothing spawns while waiting, the board does _not_ degrade during a wait — meaning there is **no natural pressure against infinite waiting** unless explicitly added. This required adding an explicit penalty structure (see reward function, Layer 3) rather than relying on emergent pressure.

Decision points occur when cooldown == 0 (which defaults to every 24 ticks after a shot, but is shifted by 6, 12, or 24 ticks if the agent chooses to wait instead of shooting). MCTS searches at decision points, not every physics tick.

---

## 5. Network Architecture

```
Spatial path:
  Conv2D(2→16, 3×3) → ReLU
  Conv2D(16→32, 3×3) → ReLU
  Conv2D(32→32, 3×3) → ReLU
  Flatten → Linear(→128)

Concatenate with 15 scalar features → [136-dim]
  → Linear(256) → ReLU → Linear(128) → ReLU

Policy head: Linear(128→64) → ReLU → Linear(64→35)   # 32 positions + 3 wait actions
Value head:  Linear(128→64) → ReLU → Linear(64→1)
```

NNUE was explicitly considered and rejected (see §2) in favor of this standard AlphaZero-style conv architecture, which fits the dense/continuous nature of the state.

---

## 6. Reward Function (FINAL — derived from akiuS's actual scoring formula)

**Resolved**: an earlier draft of this reward function used custom-designed merge/order bonus terms instead of the game's literal scoring system. That gap was caught and fixed once the real formula was supplied:

```
Merges:  Resulting Tier × 100   (T1+T1→T2: 200 ... T8+T8→T9: 900)
Orders:  Target Tier × 500      (T5: 2500, T6: 3000, T7: 3500, T8: 4000)
```

Key insight from the real formula: **orders are worth ~5× more than a merge at the same tier** (T8 merge = 800 vs T8 order = 4000). The original custom Layers 2-3 had roughly the right shape but weren't _derived_ from anything — now they are. Decision made: **Option B from the earlier draft** — use literal score deltas as the dense reward signal (replacing the old custom Layers 2-3 entirely) and keep only the T9 terminal bonus and board-management penalties as custom additions, since those aren't captured by the game's own scoring at all (T9 has no special payoff in-game; board management isn't scored until you actually lose).

```
Layer 1 — Terminal (large, sparse):
  overflow loss:              -10.0
  spill loss:                 -10.0
  tier 9 achieved:            +50.0   # custom bonus, ON TOP of the literal 
                                       # 900-pt merge score for that event — 
                                       # T9 has no special in-game payoff 
                                       # otherwise, since orders cap at T8

Layer 2 — Literal score delta (replaces old custom merge/order layers):
  every merge:                 +(Δscore × SCALE)
  every order fulfillment:     +(Δscore × SCALE)
  # Δscore pulled directly from the game's own scoring system:
  #   merges:  resulting_tier × 100
  #   orders:  target_tier × 500
  # SCALE is a normalization constant to bring these into the same rough 
  # magnitude as Layer 1's terminal rewards (±10 to ±50) — e.g. SCALE ≈ 0.01 
  # gives a T8 order (4000 pts) a reward of 40, a T1 merge (200 pts) a 
  # reward of 2. Needs empirical tuning once training starts; the point is 
  # the *ratios* between merges and orders, and between tiers, now come 
  # directly from the game's real incentive structure instead of being 
  # guessed.

Layer 3 — Continuous penalties (tiny, per-tick — UNCHANGED from before):
  proximity penalty:          -0.0005 × (max_Z_occupied / 12.0)
                               # tracks Z-axis fill toward launcher (table is 
                               # flat, no vertical stacking — see §1)
  wait penalty:                -0.001 per tick waited
  hard cap violation:          -1.0 (force-fire at center rather than true 
                                game-over, to keep episodes running for 
                                late-game data)
```

**REMOVED — old Layer 4 (exposure penalty) was cut after later review.** The original term penalized `count_t7 + count_t8` per tick whenever the order target was T7/T8, on the reasoning that "spare high-tier spheres sitting around as order fodder" should be discouraged. On reexamination this didn't hold up, for two compounding reasons:

1. **It couldn't distinguish "wasteful spare" from "necessary T9 build state."** Holding two T8s simultaneously is the literal precondition for T9 — the one strategy the whole reward function is designed to make worthwhile via the Layer 1 terminal bonus. The exposure penalty taxed every tick of that exact window, directly opposing the incentive Layer 1 was trying to create.
2. **The scenario it was actually meant to prevent can't occur under the real fulfillment rule.** Fulfillment is immediate and involuntary the instant a matching-tier sphere exists. So "three or four spare T7s piling up while a T7 order is live" is impossible — the order system itself consumes any matching sphere the moment it appears, which means there's never a window where same-tier spheres accumulate unconsumed under a live matching order. The premise the penalty was trying to discourage doesn't happen in this game.

With no remaining scenario where the term does useful work, and a clear case where it actively fights the reward function's main goal, it was cut entirely rather than reworked. This also better matches the original motivation for going AlphaZero in the first place (§2) — let the learned value network discover when spare high-tier spheres are risky vs. purposeful from outcomes, rather than hand-encoding a heuristic distinction that turned out to be wrong.

**Current final reward function is Layers 1-3 above** (terminal, literal score delta, continuous board-management penalties). No exposure/Layer 4 term remains.

**Why this is better than the original custom-tiered version**: it removes a whole axis of guesswork (relative weighting between merge tiers and order tiers) by just using the real numbers the game already defines, and it means `mean_episode_score` (an instrumentation metric, §7) and the actual reward signal are now the same underlying quantity rather than two independently-designed things that happened to be tracked side by side. The only genuinely custom incentives left are: (a) the T9 bonus, since the game doesn't reward T9 specially on its own, and (b) the board-management penalties (Z-proximity, wait), since the game has no concept of "danger" until you've already lost.

**Still open**: the SCALE constant for Layer 2 is a placeholder and needs empirical tuning once training starts — the design goal is keeping per-event rewards roughly comparable in magnitude to the Layer 1 terminal rewards (not orders-of-magnitude larger, which would make the ±10/+50 terminal signals irrelevant by comparison).

**Important historical note**: an earlier draft modeled "height" as Y-axis stacking (assuming a Suika-style gravity well with spheres piling up toward the launcher from above). This was wrong — the table is flat, spheres can't stack, Y tracking is only for detecting falls off the table edge. The real pressure axis is Z (table filling lengthwise toward the launcher). Corrected throughout.

**Tension that was raised and resolved**: initially assumed the agent needed to learn when to "protect" a T8 from being consumed by an order. Since fulfillment is involuntary, this framing was wrong — the agent can't choose whether to fulfill, only whether to have created the exposure in the first place. An early draft tried to encode this as a continuous exposure penalty (see "REMOVED" note above) but that term was later cut entirely: it couldn't distinguish wasteful spare high-tier spheres from the necessary T9-build state, and the scenario it targeted (same-tier spheres piling up under a live matching order) turns out to be impossible anyway, since involuntary fulfillment consumes a match the instant it appears. The corrected understanding — fulfillment is involuntary, so there's no "protect" lever at all — now has no dedicated reward term; it's left for the value network to learn from outcomes rather than hand-encoded.

---

## 7. Instrumentation Strategy

Two categories of metrics:

**Snapshot metrics** (computed from grid state at a given moment, cheap):

```
board_fill_percentage, front_pressure (cells near Z=12), 
back_wall_density (cells near Z=0), mid_table_density (Z 3.0-8.0),
max_tier_on_board, count_by_tier[1..8]
```

**Event metrics** (require explicit Rust-side event emission, span multiple ticks):

```
on_merge(tier, x, z, tick)
on_order_fulfilled(tier, tick)
on_sphere_created(tier, x, z, tick)
on_loss(condition, tick)
on_shot(x, tick, wait_duration)
```

**Layer 1 — Training health**: policy_loss, value_loss, entropy (should decrease _gradually_, not collapse early), mean_episode_length, **mean_episode_score** (the actual game score — was missing from an early draft, added per Cyrus's catch), score_per_order_completed (normalizes for episode length), score distribution/histogram not just mean (high mean + high variance = inconsistent brilliance, different problem than low mean + low variance).

**Layer 2 — Strategic behavior**: mean_max_tier_achieved, t8_creation_rate, t8_survival_time, mean_Z_at_shot_time, back_wall_density vs mid_table_density over training time (tests whether agent discovers the back-wall-cascade-repositioning strategy), cascade_length_distribution, wait_action_frequency (healthy range estimated ~5-15% of decision points).

**Layer 3 — T9 probe metrics**: t9_achievement_rate, **max_simultaneous_t8_count** (the single most important early indicator — does it ever even reach 2?), t8_pair_window (ticks between first and second T8 creation, should trend downward if agent learns rapid-succession strategy), order_interference_rate (how often a T8 order fires while a T8 exists on board — distinguishes "bad luck" from "systematic impossibility").

**Episode record format** (input-replay style, not full state logging):

```json
{
  "episode_id": 4721,
  "seed": 88172,
  "final_score": 3840,
  "terminal_condition": "overflow",
  "shots": [{"tick": 0, "x": -1.2}, {"tick": 31, "x": 0.8, "wait_duration": 7}, ...],
  "merges": [{"tick": 38, "resulting_tier": 3, "x": 0.7, "z": 8.2}, ...],
  "orders": [{"tick": 0, "target": 6}, {"tick": 892, "target": 7, "fulfilled_tick": 1203}, ...],
  "correction_snapshots": [{"tick": 120, "spheres": [...]}, ...]  # see §9
}
```

Reconstructing most behavioral metrics from this record in post-processing (rather than deciding all metrics upfront) is the practical goal.

Checkpointing: keep all checkpoints (not just latest) at regular intervals — every 10k steps early, every 50k once stable — to allow rollback and running the T9 probe against multiple training stages to see trend.

---

## 8. Distributed Training Architecture (K3s-specific)

Hardware: GTX 1070 (8GB VRAM, one dead CPU core) + RTX 2060 (6GB VRAM) + two CPU-only machines (one unreliable, one cheap). Headless physics benchmark: **156,377 steps/sec single-threaded** for 20 dynamic spheres (~2606× real-time at 60fps target).

**Performance math**: at 24 ticks minimum per MCTS simulation and ~400 simulations per decision (reasonable AlphaZero-scale budget), pure physics cost is trivially fast (~0.06s/decision, ~6s/game). **The real bottleneck is neural network inference, not physics** — naive synchronous per-leaf GPU queries waste most of the GPU's throughput advantage on a tiny conv-net. With batching, realistic estimate is **~100-400 self-play games/hour combined across both machines**, meaning a competent agent is plausibly trainable in roughly a week of continuous self-play (akiuS's strategic complexity is far below Go's, so likely needs far fewer total games than AlphaZero's original training).

**Batching solution — virtual loss + central inference server**:

- MCTS traverses multiple simulations "in flight" simultaneously, applying a temporary penalty (virtual loss) to in-flight leaf nodes so concurrent simulations don't redundantly explore the same path while waiting for batched network results.
- Leaves are pooled across a batch (suggested starting size 16-32) before querying the network, then results are distributed back and virtual losses removed before real backpropagation.
- **Central inference server pattern**: rather than each self-play worker running its own network instance, all workers (across both machines) submit state queries to a shared batch queue; one dedicated GPU inference service (pinned to the RTX 2060 node) batches and serves them. This pools leaves across _all_ workers for much larger effective batch sizes than any single worker could achieve alone.

**K3s mapping**:

```
inference-server (Deployment, 1 replica, GPU resource request → RTX 2060 node)
  - gRPC service, protobuf schema for (spatial_grid, scalars) → (policy, value)
  - internal batch loop with max_batch_size + max_wait_ms tuning knob 
    (~5-10ms starting point)
  - K8s service discovery: workers call it as a hostname, no manual IP mgmt

self-play-workers (Deployment, N replicas, scalable via kubectl scale)
  - spread across GTX 1070 node and CPU-only node(s)
  - each makes synchronous-looking gRPC calls; server batches internally

training process (Job or separate Deployment)
  - reads game records from shared storage
  - updates network, writes new checkpoint

Shared storage: Longhorn-backed PVC mounted into both training and 
inference-server pods for checkpoints + game records (no separate object 
store needed at this scale)
```

Open question flagged but not resolved: checkpoint reload mechanism — polling shared volume periodically (simpler, more resilient) vs. explicit admin RPC trigger from training process (more immediate, more coupling). Leaned toward **polling** as the simpler default.

---

## 9. Replay / Visualization System

Motivation: behavioral metrics and tracking output don't let Cyrus reconstruct gameplay mentally — actually _watching_ games matters, especially for evaluating whether the agent discovers nonhuman/unintuitive strategies (the whole point of the AlphaZero approach).

**Design principle**: one engine, two presentation modes — not a separate "fast" reimplementation for training that risks physics drift from the real rendered engine.

**Approach: input replay, not full state logging.** Since physics are deterministic, you only need to record the sequence of (x_position, tick) shot actions plus the RNG seed; replaying them through the same engine deterministically reproduces the full game, merges and all. Much smaller footprint than full per-tick state capture, and this is exactly what the event logging in §7 already captures — replay is nearly free given the instrumentation already planned.

**Determinism caveat (raised by Cyrus's own engine-side agent)**: true bit-for-bit determinism isn't guaranteed indefinitely even with `enhanced-determinism` + `FixedUpdate` at 60Hz, due to floating-point non-associativity across different hardware/compiler/toolchain versions over time. Solution: **hybrid replay format** — input sequence plus periodic full-state correction snapshots (e.g., every 120 ticks / 2 seconds), silently snapping sphere positions/velocities to recorded values during replay to prevent drift from compounding into visibly different cascades over thousands of ticks.

```toml
# Cargo.toml change needed in both manifests:
bevy_rapier3d = { version = "0.34.0", default-features = false, features = ["dim3", "enhanced-determinism"] }
```

Also requires: all gameplay/launcher input processed in Bevy's `FixedUpdate` schedule (not `Update`), for fixed 60Hz stepping.

**Debug mode for drift detection** (cheap addition, agreed as an "easy win"): at each correction snapshot, compute drift magnitude (`(simulated_position - recorded_position).length()`) before snapping, log a warning if it exceeds a conservative threshold (suggested starting point 0.01 units — set low initially to observe baseline rather than missing real drift). Running a batch of replays in debug mode produces a drift-magnitude distribution: tightly clustered near zero confirms the correction system is defensive-but-rarely-needed; systematic growth (especially correlated with high-tier merges, which involve larger mass/momentum values) would be a real signal worth investigating.

**Format**:

```json
{
  "seed": 88172,
  "shots": [{"tick": 0, "x": -1.2}, {"tick": 31, "x": 0.8}],
  "correction_snapshots": [
    {"tick": 120, "spheres": [{"id": 4, "tier": 3, "x": 0.71, "z": 8.21, "vx": 0.0, "vz": -0.1}, ...]}
  ]
}
```

Practical requirement: a replay mode in the existing rendered (non-headless) akiuS build that consumes this file in place of live input — feeding pre-recorded shots into the same launcher/physics code path rather than waiting for mouse clicks. Playback speed control (faster than real-time) is worth having from the start, since scrubbing through long games to find interesting moments (T8 pair windows, hail-Mary saves, odd cascades) at 1× real-time would be tedious.

**Open design choice, deliberately deferred**: exact reproduction only (just watch what happened) vs. branching replay (replay to a point, then take manual control to explore alternate actions). Recommendation was to build exact reproduction first since it's nearly free given existing logging, and treat branching as a stretch goal only if it proves useful once games are actually being watched.

---

## 10. Suggested Build Order (proposed, not yet started)

1. Headless engine + PyO3/Maturin bindings (prerequisite for everything)
2. Event logging in Rust (cheap now, painful to retrofit — do this before any training data exists that you'd want to keep)
3. Enhanced-determinism + FixedUpdate migration (needed for both training reproducibility and replay)
4. Minimal training loop in Python, just to validate bindings work end-to-end
5. Network architecture + MCTS-with-virtual-loss (the actual AlphaZero core)
6. Central inference server (gRPC) + K3s deployment manifests for self-play workers
7. Instrumentation dashboard (wandb suggested, free tier sufficient at this scale)
8. Replay mode in the rendered build, consuming the same episode records

---

## 11. Open Threads / Unresolved Questions

- Exact checkpoint reload mechanism for the inference server (polling vs. explicit trigger) — leaned toward polling, not finalized.
- Batch size and max_wait_ms tuning for the inference server are unbenchmarked guesses (16-32 batch, 5-10ms wait) — need empirical testing once the server exists.
- **Layer 2 SCALE constant** (§6) — the reward function now uses the game's literal score deltas, but the normalization constant bringing those into the same magnitude as Layer 1's terminal rewards is a placeholder and needs empirical tuning once training starts.
- The T9 outcome itself is of course completely open — this whole system exists to answer it.

---

_This document was generated as a context-window handoff partway through a design conversation. The conversation covered, in order: initial walkthrough of an AI-generated design doc, MCTS/RL/AlphaZero strategy comparison, observation/action space design (revised twice for table orientation corrections), network architecture, reward function design (revised for the Z-axis correction, the involuntary-fulfillment correction, and — after this document's first draft — corrected again to use akiuS's actual literal scoring formula instead of custom-guessed weights), instrumentation strategy, a Hatetris-comparison tangent (DAG and NNUE both ruled out as non-transferable), inference batching and K3s deployment architecture, and the replay/determinism system. No code has been written yet — this is purely the design phase._

---

Here's the real literature/concepts behind each major choice, organized so you can go read the actual sources rather than relying on me.

---

## Core algorithm

- **The AlphaZero paper itself** — Silver et al., _"A General Reinforcement Learning Algorithm that Masters Chess, Shogi, and Go through Self-Play"_ (2018), and the earlier AlphaGo Zero paper. This is the actual source for the policy/value network + MCTS bootstrapping loop. You should be able to point to this directly rather than to our conversation.
- **MCTS itself** — Kocsis & Szepesvári's UCT paper (2006) is the canonical citation for the upper-confidence-bound tree search that MCTS is built on. Worth knowing UCB1/UCT by name since "MCTS" is really a family of techniques and UCT is the specific selection rule most implementations use.
- **PUCT** (the specific MCTS variant AlphaZero uses, which weights exploration by the policy network's prior) — this is what actually justifies "policy network narrows the search," and it's a specific formula, not just an intuition. Worth reading the formula directly rather than my paraphrase.

## Reward design

- **Reward shaping** — Ng, Harada, Russell, _"Policy Invariance Under Reward Transformations"_ (1999). This is the actual theoretical grounding for "you can add intermediate rewards without changing the optimal policy, _if_ you do it right (potential-based shaping)." Relevant because our reward function has multiple layers, and this paper is the formal justification for why that's safe (or the conditions under which it isn't).
- **Reward hacking / specification gaming** — this is a well-documented empirical phenomenon, not just an abstract worry. Worth reading Victoria Krakovna's specification gaming examples list (a running catalog of real cases across many RL projects) — this is what grounds "agents exploit gaps between proxy and intent," with concrete precedent rather than just my saying it could happen.
- **Sparse vs. dense reward tradeoffs** — this connects to the broader **exploration problem** in RL, worth knowing the term "credit assignment problem" specifically, since that's the formal name for "the agent got a reward, which earlier action caused it."

## Network architecture

- **Why convolutions for spatial state** — this is standard CNN literature (LeCun et al.), but the specific justification for "use a grid representation for board games" goes back to how AlphaGo/AlphaZero represent Go boards as image-like tensors. Worth being able to say "the spatial representation follows the precedent set in the AlphaGo Zero architecture section" rather than "it seemed reasonable."
- **Batching/virtual loss** — this is from a specific paper, Chaslot et al. or more directly the parallelization techniques used in the original AlphaZero implementation details (Silver et al. supplementary material covers their actual batching approach). Worth tracking down the specific section rather than treating it as something we derived from first principles together — it wasn't; it's known engineering practice Claude described from memory.

## The thing I'd flag most

If someone pushes hard on "why this and not that," the most honest and defensible answer for several of the choices (reward scaling constants, grid resolution, batch size) is **"this was a starting point grounded in general practice, not yet empirically validated for this specific game — here's the ablation/tuning process I'd run to actually justify it."** That's a stronger answer than pretending the numbers were rigorously derived, because they weren't — several (SCALE constant, drift threshold, batch size) were explicitly flagged as placeholders in our conversation.

A good general framing to bring to a new conversation, if you want to firm this up further: **"what's the actual empirical/theoretical justification for X, as opposed to a plausible-sounding default"** — that question, asked per design choice, will surface which parts of this design rest on real precedent (the core algorithm, the architecture pattern) versus which parts are reasonable-but-unvalidated engineering guesses (most of the specific constants).


---

## What "ablation" actually means here, concretely

The general pattern: hold everything else fixed, vary one thing, measure the effect on a small set of behavioral metrics you've already instrumented (§7 of the handoff — you built this for exactly this purpose). The trap to avoid is changing multiple things between runs and not being able to attribute the effect to any one of them.

---

## 1. The Layer 2 SCALE constant (reward magnitude)

This is the most clearly "needs a real process" item, because it's currently a guess (~0.01) with no empirical basis at all.

**What you're actually tuning:** the ratio between the dense literal-score signal and the sparse terminal signals (±10 loss, +50 T9). Too large and the terminal rewards become noise the agent barely notices; too small and there's no gradient signal until something terminal happens, which reintroduces the original sparse-reward problem this layer was meant to solve.

**Concrete ablation:** run short training sessions (not full training — just enough to see early-training entropy and episode-length trends, maybe a few thousand episodes) at 3-4 SCALE values spanning an order of magnitude each direction (e.g. 0.001, 0.01, 0.1). Watch:

- **Entropy decay rate** (§7 Layer 1) — if entropy collapses very fast at high SCALE, that's a sign score-chasing is dominating early exploration before the agent has any real information.
- **Does the agent ever experience a T9 or loss event with the value estimate "noticing"?** A crude proxy: at low SCALE, does value loss spike noticeably after rare terminal events (indicating the network is still being meaningfully surprised by them), or do terminal events look like rounding noise against the accumulated dense reward?

You're looking for the SCALE value where dense reward provides gradient without drowning out the terminal signal — there isn't a closed-form answer, this is genuinely an empirical sweep.

---

## 2. Grid resolution (16×28 cells)

This one actually has a cleaner theoretical lower bound than most of the other choices, which makes the ablation more targeted rather than a blind sweep.

**The lower bound you already have:** cell size must be ≤ smallest sphere diameter (1.0 unit) or you risk a cell ambiguously representing parts of two different spheres. 0.5-unit cells satisfy this with margin.

**What's actually unvalidated:** whether finer resolution (e.g. 0.25-unit cells, 32×56 grid) meaningfully helps the value network distinguish board states that matter for T9 — specifically, whether the "two T8s side by side with only 0.2 units of clearance" geometry needs finer spatial resolution to be represented accurately, or whether 0.5-unit cells already capture it adequately since each T8 already spans ~7-8 cells either way.

**Concrete ablation:** train two otherwise-identical setups at 16×28 vs 32×56 for a fixed compute budget (not fixed episode count — fixed _wall-clock or GPU-hours_, since the finer grid costs more per forward pass) and compare `max_simultaneous_t8_count` achievement rate and `t9_achievement_rate` at matched compute. If the coarser grid reaches the same milestones at the same compute cost, the extra resolution isn't earning its keep and you should keep 16×28. This is the kind of ablation where "no difference" is itself a useful, fully concrete answer — it tells you the original choice was already sufficient.

---

## 3. MCTS simulation budget (400 sims/decision) and batch size (16-32)

These two are coupled and should probably be ablated together rather than separately, since batch size affects how cheaply you can afford a given simulation count.

**What you're tuning:** the tradeoff between search depth/quality per move (more simulations = better-informed decisions, closer to "true" MCTS value) and games-per-hour throughput (fewer simulations = more self-play data per unit time). This is a classic compute-allocation tradeoff in AlphaZero-style systems — there's actual literature on this exact question (the original AlphaZero paper's appendix discusses simulation count vs. training speed tradeoffs) but the _correct_ answer is dataset/domain-specific, hence empirical.

**Concrete ablation:** fix a wall-clock budget (say, 2 hours of self-play), run separate batches at 100/400/800 simulations per decision, and compare not loss curves but **games generated per hour** against **per-game quality proxy** (e.g. does a 100-sim self-play game make obviously worse decisions on replay review than an 800-sim one, judged by you watching a handful of each via the replay system). If 100-sim games are nearly as good per-decision but 4x more numerous, more simulations isn't worth the throughput cost — more (lower-quality) data may train better than less (higher-quality) data, which is a real empirical finding from the original AlphaZero/AlphaGo Zero ablations, not a given.

For batch size specifically: this is more of a pure systems-benchmarking question than a learning-quality one — sweep 8/16/32/64 and just measure wall-clock latency per decision on your actual hardware, independent of training quality at all. This is the cheapest ablation in the whole list since it doesn't require training runs, just timing the inference server under load.

---

## 4. Drift warning threshold (0.01 units)

This is the easiest one to do properly, and it's not really a tuning question so much as a measurement question — you're not optimizing for a tradeoff, you're calibrating a threshold against an observed distribution.

**Process:** run a batch of replays (say, 50-100) in debug mode with the threshold set artificially low (e.g. 0.0001, low enough to log essentially everything) purely to collect the full drift distribution, not to actually warn usefully. Plot the distribution. Set the real threshold at something like the 95th or 99th percentile of normal/expected drift, so the warning fires on genuine outliers rather than routine floating-point noise. This is a one-time calibration exercise rather than an ongoing tuning loop — once you've characterized the distribution, the threshold should be stable.

---

## 5. Wait penalty / hard cap (-0.001 per tick, 480-tick cap)

**What you're tuning:** whether the wait penalty successfully prevents stalling (the original concern — nothing punishes infinite waiting except this term) without discouraging legitimate hail-Mary waits.

**Concrete ablation:** this is best evaluated behaviorally, not via loss curves — track `wait_action_frequency` (§7 Layer 2) over training at 2-3 penalty magnitudes. If frequency trends toward 0% at high penalty, you've over-corrected (agent never uses the hail-Mary tool at all, which you specifically designed the action space to support). If frequency stays high/doesn't decrease over training, the penalty isn't doing its job and stalling persists. The 5-15% "healthy range" estimate from earlier in this conversation was itself just an intuition-based guess, not derived from anything — worth treating it as a hypothesis to confirm or revise once you have real training curves, not a target to hit.

---

## The general principle underneath all of this

Notice the pattern: almost none of these ablations require _full_ training runs to get a useful signal. Short runs, fixed compute/wall-clock budgets, and behavioral metrics (which you specifically built instrumentation for) get you most of the way. The expensive mistake would be running one full multi-day training session per candidate value — you don't need that resolution to catch most of the bad choices, and the instrumentation layer is specifically what makes the cheap, early-signal version of this possible instead.

