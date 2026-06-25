import torch
import torch.nn as nn
import torch.nn.functional as F


class AkiusNet(nn.Module):
    def __init__(self):
        super().__init__()

        # Spatial path: 3 input channels — tier, vel_z, speed
        # Each Conv2d(kernel_size=3) with no padding shrinks both spatial dims by 2.
        # Starting from 16×28:
        #   after conv1: 16 channels, 14×26
        #   after conv2: 32 channels, 12×24
        #   after conv3: 32 channels, 10×22
        #   flatten:     32 × 10 × 22 = 7040
        self.conv1 = nn.Conv2d(3, 16, kernel_size=3)
        self.conv2 = nn.Conv2d(16, 32, kernel_size=3)
        self.conv3 = nn.Conv2d(32, 32, kernel_size=3)
        self.conv_linear = nn.Linear(7040, 128)

        # Shared trunk: conv output (128) + scalars (15) = 143
        self.shared1 = nn.Linear(143, 256)
        self.shared2 = nn.Linear(256, 128)

        # Policy head — 35 logits (32 shoot positions + 3 wait durations)
        self.policy_hidden = nn.Linear(128, 64)
        self.policy_out = nn.Linear(64, 35)

        # Value head — single scalar estimate of position quality
        self.value_hidden = nn.Linear(128, 64)
        self.value_out = nn.Linear(64, 1)

    def forward(self, grid, scalars):
        # grid arrives as [B, 16, 28, 3] — channels last (from numpy)
        # Conv2d needs channels first: [B, 3, 16, 28]
        x = grid.permute(0, 3, 1, 2)        # [B, 3, 16, 28] # the observation comes out of Rust as [batch, height, width, channels] (channels last, because that's how numpy arrays from the game are laid out). PyTorch conv layers expect [batch, channels, height, width]. The numbers in permute are the new order of axes — keep axis 0 (batch), move axis 3 (channels) to position 1, then axes 1 and 2 stay as height and width.               

        x = F.relu(self.conv1(x))           # [B, 16, 14, 26]
        x = F.relu(self.conv2(x))           # [B, 32, 12, 24]
        x = F.relu(self.conv3(x))           # [B, 32, 10, 22]
        x = x.flatten(start_dim=1)          # [B, 7040]
        x = F.relu(self.conv_linear(x))     # [B, 128]

        # Concatenate conv output with scalar features
        x = torch.cat([x, scalars], dim=1)  # [B, 143] # merge point — it glues the 128-wide conv output and the 15 scalars side by side into a single 143-wide vector. dim=1 means "along the second dimension" (not across the batch).   
        x = F.relu(self.shared1(x))         # [B, 256]
        x = F.relu(self.shared2(x))         # [B, 128]

        policy = F.relu(self.policy_hidden(x))  # [B, 64]
        policy = self.policy_out(policy)         # [B, 35] — raw logits, no softmax

        value = F.relu(self.value_hidden(x))    # [B, 64]
        value = self.value_out(value)            # [B, 1]

        return policy, value.squeeze(-1)         # [B, 35], [B]
