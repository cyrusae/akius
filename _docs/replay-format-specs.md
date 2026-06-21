# akiuS Replay/Record Format — Implementation Reference

Scope: just the replay system (input-replay + drift-correction snapshots + debug mode). Pulled out of a larger design conversation for handoff to a coding agent. Game context (table dims, physics, scoring) is in the full handoff doc if needed, but shouldn't be required for this subset.

---

## Design Principle

One engine, two presentation modes — not a separate "fast" reimplementation for training that risks physics drift from the real rendered engine. Headless training generates a record; the existing rendered build consumes that record in a replay mode. Do not build a second physics path.

## Core Approach: Input Replay, Not Full State Logging

Physics are deterministic (see determinism requirements below), so a replay only needs:
- the RNG seed (controls dispenser tier sequence + order rolls)
- - the sequence of shot inputs (tick + X position)
Replaying these through the same engine from the same seed deterministically reproduces the full game — merges, cascades, everything — without needing to log full sphere state every tick. This is the same approach as input-replay systems in fighting games / RTS games.

---

## Determinism Requirements (prerequisite, must be in place first)

1. **Enable `enhanced-determinism`** on `bevy_rapier3d` in both Cargo manifests:
2.    ```toml
3.    bevy_rapier3d = { version = "0.34.0", default-features = false, features = ["dim3", "enhanced-determinism"] }
4.    ```
5. 2. **All gameplay/launcher input must be processed in Bevy's `FixedUpdate` schedule** (60Hz fixed step), not `Update`. This is required for tick-aligned determinism, not optional.
6. 3. **Known limitation**: even with the above, true bit-for-bit determinism is not guaranteed indefinitely — floating-point non-associativity means tiny rounding differences can creep in across different hardware, compiler versions, or toolchain updates over time. This is *not* a bug to chase down; it's why the format below includes correction snapshots rather than relying on pure input-replay alone. Over short replays it's invisible; over thousands of ticks with cascading merges, uncorrected drift can compound into a visibly different game (a merge trigger firing one tick early/late cascades into different subsequent physics).
---

## Record Format

```json
{
  "seed": 88172,
  "shots": [
    {"tick": 0, "x": -1.2},
    {"tick": 31, "x": 0.8},
    {"tick": 79, "x": -0.3}
  ],
  "correction_snapshots": [
    {
      "tick": 120,
      "spheres": [
        {"id": 4, "tier": 3, "x": 0.71, "z": 8.21, "vx": 0.0, "vz": -0.1},
        {"id": 7, "tier": 2, "x": -1.8, "z": 4.3, "vx": 0.2, "vz": 0.0}
      ]
    },
    {"tick": 240, "spheres": [ ... ]}
  ]
}
```

**`shots`**: every player action, in tick order. This is the actual input replay data.

**`correction_snapshots`**: full sphere state (position + velocity per sphere, by stable id) captured periodically — **every 120 ticks (2 seconds) at 60Hz** is the suggested starting interval. Frequent enough that drift can't meaningfully compound before being corrected; infrequent enough to keep the file small. Could likely go coarser (e.g. every 5s) without issue, but the data is cheap either way so there's no strong reason to optimize this down.

Sphere `id` needs to be a stable identifier that survives merges predictably (or the snapshot only needs to cover spheres that exist at that tick — new ids post-merge are fine, the point is matching simulated-vs-recorded state per existing sphere at correction time).

---

## Replay Playback Behavior

1. Load seed, initialize RNG identically to a live game.
2. 2. Step physics tick by tick.
3. 3. At each tick matching a `shots` entry, inject that shot (same launcher/physics code path as live player input — do not special-case replay input handling separately from normal input handling beyond swapping the input *source*).
4. 4. At each tick matching a `correction_snapshots` entry, **before** continuing simulation: compare current simulated sphere state to the recorded snapshot, then forcibly set position + velocity to the recorded values (see debug mode below for what to do with the comparison).
5. 5. Continue until the recorded game's terminal tick.
**Playback speed control should be supported from the start** — scrubbing through a multi-thousand-tick game at 1× real-time to find a specific moment (e.g. a T8 pair window, a hail-Mary save) is impractical. Doesn't need to be fancy, just needs a speed multiplier or seek.

---

## Debug Mode (drift detection)

Cheap to add since the simulated-vs-recorded comparison already has to happen to perform the correction — debug mode just means *logging* the delta instead of silently discarding it.

```rust
fn apply_correction_snapshot(
    sphere: &mut Sphere,
    recorded: &SphereSnapshot,
    debug_mode: bool,
) {
    if debug_mode {
        let drift = (sphere.position - recorded.position).length();
        if drift > DRIFT_WARNING_THRESHOLD {
            warn!(
                "Sphere {} drifted {:.4} units at tick {} (tier {})",
                sphere.id, drift, current_tick, sphere.tier
            );
        }
    }
    sphere.position = recorded.position;
    sphere.velocity = recorded.velocity;
}
```

- `DRIFT_WARNING_THRESHOLD`: start conservative/low (suggested **0.01 units**) so you actually observe the baseline drift magnitude rather than missing it because the threshold was pre-tuned for a problem you haven't measured yet. Loosen later once you know what normal looks like.
- - Goal of running a batch of replays in debug mode: get a distribution of drift magnitudes across many games. Tightly clustered near zero → correction system is defensive-but-rarely-needed, fine to stop worrying about it. Drift that grows with episode length, or correlates with specific events (e.g. high-tier merges, which involve larger mass/momentum values and more floating-point ops per collision) → a real signal worth investigating further, not just papering over with snapshots.
---

## Deferred / Not In Scope Yet

**Branching replay** (replay to a point, then take manual control to explore alternate actions from there) was discussed and deliberately deferred. Build exact-reproduction-only replay first — it's nearly free given the format above. Branching is a stretch goal only if it proves useful once games are actually being watched; it requires the replay system to hand control back to live input mid-playback, which is meaningfully more engineering than what's specified here.

---

## Source

This is a narrowed extract of a larger design conversation (full handoff: `akiuS_alphazero_handoff.md`, §9). That document also covers the AlphaZero training architecture this replay system supports (episode records from self-play already contain the `shots` sequence needed here — replay and training instrumentation share the same underlying log).
