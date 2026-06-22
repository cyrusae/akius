# akiuS AlphaZero — Addendum: Reward Revision + Validation Process

Short follow-up to `akiuS_alphazero_handoff.md`. Covers two things resolved after that document was written: a flaw found and fixed in the reward function, and a conversational discussion of how to actually validate/tune the placeholder constants left open in §11 of the main doc. Read alongside the main handoff, not standalone.

---

## 1. Reward Function Correction: Exposure Penalty Removed

The main handoff's §6 originally included a "Layer 4/5 exposure penalty": `-0.0002 × (count_t7 + count_t8)` per tick, active only when the order target was T7/T8. Intent was to discourage leaving spare high-tier spheres around as order fodder.

**This was cut.** Two compounding problems surfaced on review:

1. **It penalized the literal precondition for T9.** Holding two T8s simultaneously is required to attempt the T9 merge — exactly the scenario Layer 1's +50 terminal bonus is designed to make worth the risk. The exposure penalty taxed every tick of that window, directly opposing the incentive structure's main goal.
2. **The scenario it targeted can't happen under the real rules.** Fulfillment is immediate and involuntary the instant a matching sphere exists (per the original game design — see main handoff §1). So "several spare T7s piling up while a T7 order is live" is structurally impossible: the order system itself consumes any match the moment it appears. There's no window where same-tier spheres accumulate unconsumed under a live matching order, so the penalty's stated justification never actually applied.

**Current reward function is Layers 1-3 only**: terminal (loss/T9), literal score delta (merge tier × 100, order tier × 500, scaled), and continuous board-management penalties (Z-proximity, wait, hard-cap). No exposure term. The `count_t7`/`count_t8` observation-space features (§3 of the main doc) stay — they're still useful input for the value network to learn the real distinction (purposeful vs. wasteful high-tier holding) from outcomes, rather than that distinction being hand-coded and wrong.

This is also a small case study worth keeping in mind generally: it was the one reward term added by analogy/intuition rather than derived from either the game's actual scoring system or a clean terminal condition, and it was also the one that didn't hold up. Reward terms that aren't traceable to either real game mechanics or an explicit terminal goal are the ones to be most suspicious of.

---

## 2. Validation/Ablation Process for Placeholder Constants

The main handoff (§11) flags several constants as unbenchmarked placeholders: the Layer 2 SCALE constant, grid resolution, MCTS simulation budget + batch size, drift warning threshold, and wait penalty magnitude. None of these need full multi-day training runs to get a useful signal — short runs at fixed compute/wall-clock budgets plus the existing behavioral instrumentation (main doc §7) cover most of it.

**General pattern**: hold everything fixed except one variable, run short comparisons, read off behavioral metrics you've already instrumented — not loss curves alone, and not replay-watching as the primary comparison method (see correction below).

- **SCALE constant**: sweep ~3-4 values across an order of magnitude (e.g. 0.001/0.01/0.1) for a few thousand episodes each. Watch entropy decay rate and whether terminal events (T9, loss) still visibly affect value loss, or get drowned out by accumulated dense reward.
- **Grid resolution (16×28 vs. finer)**: train at matched _compute/wall-clock_ (not matched episode count, since finer grids cost more per forward pass) and compare `max_simultaneous_t8_count` and `t9_achievement_rate`. "No difference" is a fully valid, useful outcome — confirms the coarser grid was already sufficient.
- **MCTS sim budget + batch size**: fix a wall-clock self-play budget, compare games-generated-per-hour against a quality proxy at different sim counts (100/400/800). Batch size alone (8/16/32/64) is a pure latency-benchmarking question on the inference server, no training run needed at all.
- **Drift warning threshold**: not a tradeoff to optimize, just a one-time calibration — run replays in debug mode with the threshold set very low to collect the full drift distribution, then set the real threshold around the 95th-99th percentile of normal/expected drift.
- **Wait penalty / hard cap**: track `wait_action_frequency` over training at 2-3 penalty magnitudes. Frequency trending to ~0% = over-corrected (hail-Mary tool never used). Frequency staying high = penalty isn't working. The "5-15% healthy range" mentioned earlier in the broader conversation was itself an unvalidated guess, not a target — treat it as a hypothesis to check against real curves, not a goal to hit.

---

## 3. Correction: Score Is the Primary Evaluation Metric, Not Replay Review

An earlier framing leaned on "watch replays and judge quality" as the comparison method for things like the MCTS simulation-budget ablation. That's the wrong primary tool — it's expensive (your time) and a cheaper, more scalable signal already exists in the instrumentation layer.

**Correct framing**: `mean_episode_score`, **conditioned on terminal outcome** — split into "terminated via loss" vs. "terminated via T9" and compare score distributions _within_ each bucket separately, not pooled. Pooling hides exactly the distinction that matters (a run with high average score from surviving long mediocre games looks identical, pooled, to one reaching T9 more often but scoring less per game otherwise) — same reasoning as the existing score-per-order-completed metric in the main doc's §7, just applied one level up to outcome-conditioned comparisons between ablation settings.

**Replay review's actual role**: diagnostic, not evaluative. Use it _after_ the score/T9-rate comparison flags something worth investigating ("why is the agent doing this specific thing"), not as the primary method for deciding which setting is better. Score answers "which is better"; replay answers "why."

---

_Addendum generated at the tail end of the same design conversation as the main handoff, after token budget was flagged as past ~80%. Covers: the exposure-penalty removal (with full reasoning preserved, not just the final state), a conversational walkthrough of ablation processes for each placeholder constant, and a correction to the evaluation methodology favoring outcome-conditioned score over replay review as the primary comparison tool._
