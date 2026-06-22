import numpy as np
import akius_train

def test_py_game_env():
    print("Initializing PyGameEnv...")
    env = akius_train.PyGameEnv()
    print("Environment initialized successfully!")

    print("\nTesting reset()...")
    # Reset the environment with a seed
    obs, info = env.reset(seed=42)
    
    # Check that obs is a tuple of (grid, scalars)
    assert isinstance(obs, tuple) and len(obs) == 2, f"Expected obs to be a 2-tuple, got {type(obs)}: {obs}"
    grid, scalars = obs
    
    print(f"Grid type: {type(grid)}, shape: {grid.shape if hasattr(grid, 'shape') else 'N/A'}")
    print(f"Scalars type: {type(scalars)}, shape: {scalars.shape if hasattr(scalars, 'shape') else 'N/A'}")
    print(f"Info dict: {info}")
    
    # Verify shape sizes
    assert grid.shape == (16, 28, 3), f"Expected grid shape (16, 28, 3), got {grid.shape}"
    assert scalars.shape == (15,), f"Expected scalars shape (15,), got {scalars.shape}"
    
    # Check initial dispenser queue from reset: current=1, next=2
    print(f"Initial dispenser current: {scalars[0]}, next: {scalars[1]}")
    assert scalars[0] == 1.0, f"Expected current=1, got {scalars[0]}"
    assert scalars[1] == 2.0, f"Expected next=2, got {scalars[1]}"

    # Check active order (target tier = 6)
    print(f"Active order target tier: {scalars[2]}")
    assert scalars[2] == 6.0, f"Expected target_tier=6, got {scalars[2]}"

    # Test taking a step with action 16 (Shoot at middle, X=0)
    print("\nTesting step(action=16) [Shoot at middle]...")
    next_obs, reward, terminated, truncated, step_info = env.step(16)
    
    grid, scalars = next_obs
    print(f"Reward: {reward}")
    print(f"Terminated: {terminated}")
    print(f"Truncated: {truncated}")
    print(f"Step info: {step_info}")
    
    # Check step info fields
    assert "score_delta" in step_info, "score_delta missing from step_info"
    assert "ticks_simulated" in step_info, "ticks_simulated missing from step_info"
    assert "accumulated_z_pressure" in step_info, "accumulated_z_pressure missing from step_info"
    assert "win_condition_met" in step_info, "win_condition_met missing from step_info"
    assert "loss_condition_met" in step_info, "loss_condition_met missing from step_info"
    
    assert step_info["ticks_simulated"] == 24.0, f"Expected 24 ticks for shoot action, got {step_info['ticks_simulated']}"

    # Test taking a step with action 32 (Wait 6 ticks)
    print("\nTesting step(action=32) [Wait 6 ticks]...")
    next_obs, reward, terminated, truncated, step_info = env.step(32)
    print(f"Step info: {step_info}")
    assert step_info["ticks_simulated"] == 6.0, f"Expected 6 ticks for action 32, got {step_info['ticks_simulated']}"

    print("\nAll checks passed successfully!")

if __name__ == "__main__":
    test_py_game_env()
