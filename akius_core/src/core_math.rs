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
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn test_radius_scaling() {
        // Exact expected radii for tiers 1 to 9
        let expected_radii = [
            (1, 0.5),
            (2, 0.605),
            (3, 0.73205),
            (4, 0.8857805),
            (5, 1.0717944),
            (6, 1.2968712),
            (7, 1.5692142),
            (8, 1.8987492),
            (9, 2.2974865),
        ];

        for &(tier, expected) in &expected_radii {
            let actual = get_radius(tier);
            assert!(
                (actual - expected).abs() < 1e-6,
                "Expected radius for tier {} to be {}, got {}",
                tier,
                expected,
                actual
            );
        }

        // Out-of-bounds values should clamp gracefully to tier 1 and tier 9 respectively
        assert_eq!(get_radius(0), get_radius(1));
        assert_eq!(get_radius(10), get_radius(9));
    }

    #[test]
    fn test_scoring_math() {
        // Test all merge points from resulting tiers 1 to 9
        for tier in 1..=9 {
            let expected_points = tier as u32 * 100;
            assert_eq!(
                get_merge_points(tier),
                expected_points,
                "Merge points mismatch for tier {}",
                tier
            );
        }

        // Test all order points from target tiers 1 to 9
        for tier in 1..=9 {
            let expected_points = tier as u32 * 500;
            assert_eq!(
                get_order_points(tier),
                expected_points,
                "Order points mismatch for tier {}",
                tier
            );
        }
    }

    #[test]
    fn test_weighted_dispenser_distribution() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut counts = [0u32; 5];
        let iterations = 10_000;

        for _ in 0..iterations {
            let tier = get_random_dispensed_tier(&mut rng);
            assert!(
                (1..=4).contains(&tier),
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
