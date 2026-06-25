from __future__ import annotations
import math
import numpy as np
import torch 
from dataclasses import dataclass, field
from .network import AkiusNet

@dataclass
class MCTSNode:
 prior: float = 0.0
 visit_count: int = 0
 value_sum: float = 0.0
 children: dict[int, MCTSNode] = field(default_factory=dict)
 
 @property
 def Q(self) -> float:
  if self.visit_count == 0:
   return 0.0
  return self.value_sum / self.visit_count
 
 @property
 def is_expanded(self) -> bool:
  return len(self.children) > 0
 
 def puct_score(self, parent_visits: int, c_puct: float) -> float:
  u = c_puct * self.prior * math.sqrt(max(1, parent_visits)) / (1 + self.visit_count)
  return self.Q + u
 
 def select_child(self, c_puct: float) -> tuple[int, MCTSNode]:
  best_action = max(
   self.children, 
   key=lambda a: self.children[a].puct_score(self.visit_count, c_puct)
  )
  return best_action, self.children[best_action]
 
 def expand(self, policy_probs: np.ndarray):
  for action in range(35): 
   self.children[action] = MCTSNode(prior=policy_probs[action])

  