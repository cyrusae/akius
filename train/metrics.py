from dataclasses import dataclass, field 
from .reward import StepInfo

@dataclass 
class EpisodeMetrics:
 total_score: float = 0.0
 total_ticks: int = 0
 total_steps: int = 0
 wait_steps: int = 0
 hard_cap_violations: int = 0

 total_merges: int = 0
 t8_creations: int = 0
 t9_achieved: bool = False 
 max_simultaneous_t8: int = 0
 cascade_lengths: list[int] = field(default_factory=list)
 # cascade_lengths: list[int] = [] would be wrong — Python would share the same list across every instance of
 # the class. field(default_factory=list) tells it "call list() fresh for each new instance."

 completed_orders: int = 0

def update(self, action: int, obs: tuple, info: StepInfo) -> None:
 _grid, scalars = obs

 self.total_score += info["score_delta"]
 self.total_ticks += int(info["ticks_simulated"])
 self.total_steps += 1

 # note when it chose to wait 
 if action >= 32:
  self.wait_steps += 1
 if info["hard_cap_violated"]:
  self.hard_cap_violations += 1
 
 # Merge tracking
 step_merge_count = len(info["merges"])
 self.total_merges += step_merge_count
 self.cascade_lengths.append(step_merge_count)
 for merge in info["merges"]:
  if merge["tier"] == 8:
   self.t8_creations += 1
  if merge["tier"] == 9:
   self.t9_achieved = True 
  
 # Order tracking
 self.completed_orders += len(info["fulfillments"])

 # scalars[7] is count_t8 on board right now
 current_t8_count = int(scalars[7])
 self.max_simultaneous_t8 = max(self.max_simultaneous_t8, current_t8_count)

 if info["win_condition_met"]:
  self.t9_achieved = True 


def summary(self) -> dict[str, float]:
 return {
  "episode/score": self.total_score,
  "episode/length_ticks": self.total_ticks,
  "episode/completed_orders": self.completed_orders,
  "episode/score_per_order": (
   self.total_score / self.completed_orders
   if self.completed_orders > 0 else 0.0
   ),
  "episode/wait_frequency": (
   self.wait_steps / self.total_steps
   if self.total_steps > 0 else 0.0
   ),
  "episode/hard_cap_violations": self.hard_cap_violations,
  "episode/t8_creations": self.t8_creations,
  "episode/max_simultaneous_t8": self.max_simultaneous_t8,
  "episode/t9_achieved": float(self.t9_achieved),
  "episode/total_merges": self.total_merges,
  "episode/mean_cascade_length": (
   sum(self.cascade_lengths) / len(self.cascade_lengths)
   if self.cascade_lengths else 0.0
   ),
 }