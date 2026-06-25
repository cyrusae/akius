from typing import TypedDict

# Tunable constants
REWARD_SCALE = 0.01 # This is a glorified placeholder!
PROXIMITY_PENALTY = 0.0005 # Stay away from the edge 
WAIT_PENALTY_PER_TICK = 0.001 # Make time pass 
LOSS_REWARD = -10.0 # Losing the game 
T9_REWARD = 50.0 # The win condition
HARD_CAP_PENALTY = -1.0 # Penalty for not taking explicit action

# Match what's exported from Rust

class MergeEvent(TypedDict):
 tick: int
 tier: int
 x: float 
 z: float 

class FulfillmentEvent(TypedDict):
 tick: int 
 tier: int 

class StepInfo(TypedDict):
 score_delta: float 
 ticks_simulated: float
 accumulated_z_pressure: float 
 win_condition_met: bool
 loss_condition_met: bool 
 hard_cap_violated: bool
 merges: list[MergeEvent]
 fulfillments: list[FulfillmentEvent]

# This isn't being enforced by anything
# It's my job to keep up with it 


def compute_reward(info: StepInfo) -> float:
 reward = 0.0

 # Layer 2: literal score delta, normalized
 reward += info["score_delta"] * REWARD_SCALE

 # Layer 3: proximity penalty accumulated across all ticks this step
 reward -= PROXIMITY_PENALTY * info["accumulated_z_pressure"]

 # Layer 3: time penalty across all steps (including cooldown)
 reward -= WAIT_PENALTY_PER_TICK * info["ticks_simulated"]

 # Layer 1: terminal signals
 if info["loss_condition_met"]:
  reward += LOSS_REWARD
 if info["win_condition_met"]:
  reward += T9_REWARD
 
 # Hard cap: force shot fired to prevent stalling, episode continues
 if info["hard_cap_violated"]:
  reward += HARD_CAP_PENALTY

 return reward