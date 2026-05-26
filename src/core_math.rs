/// Returns the physical radius of a sphere given its tier (1-9).
/// Clamps input to the valid range [1, 9].
pub fn get_radius(tier: u8) -> f32 {
    let clamped_tier = tier.clamp(1, 9);
    // Tier 1 radius = 0.5. Each subsequent tier scales by 1.21.
    0.5 * 1.21f32.powi(clamped_tier as i32 - 1)
}

/// Returns the score points awarded for a successful merge resulting in `resulting_tier`.
pub fn get_merge_points(resulting_tier: u8) -> u32 {
    resulting_tier as u32 * 100
}

/// Returns the flat score points awarded for completing an order of `target_tier`.
pub fn get_order_points(target_tier: u8) -> u32 {
    target_tier as u32 * 500
}

/// Selects a random sphere tier (1-4) for dispensation based on weighted probabilities:
/// - Tier 1: 40%
/// - Tier 2: 32%
/// - Tier 3: 18%
/// - Tier 4: 10%
pub fn get_random_dispensed_tier<R: rand::Rng>(rng: &mut R) -> u8 {
    let roll = rng.random_range(0..100);
    match roll {
        0..40 => 1,
        40..72 => 2,
        72..90 => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn test_radius_scaling() {
        // Tier 1 should be exactly 0.5
        assert_eq!(get_radius(1), 0.5);

        // Tier 9 should be approx 2.30
        let r9 = get_radius(9);
        assert!(
            (r9 - 2.298).abs() < 0.01,
            "Expected r9 to be ~2.30, got {}",
            r9
        );

        // Out-of-bounds values should clamp gracefully
        assert_eq!(get_radius(0), get_radius(1));
        assert_eq!(get_radius(10), get_radius(9));
    }

    #[test]
    fn test_scoring_math() {
        assert_eq!(get_merge_points(1), 100);
        assert_eq!(get_merge_points(5), 500);

        assert_eq!(get_order_points(3), 1500);
        assert_eq!(get_order_points(6), 3000);
    }

    #[test]
    fn test_weighted_dispenser_distribution() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut counts = [0u32; 5];
        let iterations = 10_000;

        for _ in 0..iterations {
            let tier = get_random_dispensed_tier(&mut rng);
            assert!(
                tier >= 1 && tier <= 5,
                "Dispensed tier out of bounds: {}",
                tier
            );
            counts[tier as usize] += 1;
        }

        let p1 = counts[1] as f32 / iterations as f32;
        let p2 = counts[2] as f32 / iterations as f32;
        let p3 = counts[3] as f32 / iterations as f32;
        let p4 = counts[4] as f32 / iterations as f32;

        // Verify probabilities are close to target weights (within 2% margin)
        assert!((p1 - 0.40).abs() < 0.02, "Tier 1: {}, expected ~0.40", p1);
        assert!((p2 - 0.32).abs() < 0.02, "Tier 2: {}, expected ~0.32", p2);
        assert!((p3 - 0.18).abs() < 0.02, "Tier 3: {}, expected ~0.18", p3);
        assert!((p4 - 0.10).abs() < 0.02, "Tier 4: {}, expected ~0.10", p4);
    }
}
