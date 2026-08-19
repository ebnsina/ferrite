//! Perceptual hashing. Pure arithmetic, so it is testable without a video file.
//!
//! dHash survives re-encode, rescale, crop, watermark and letterboxing, which
//! an exact SHA-256 does not — which is the whole point of having both.

/// Width of the greyscale buffer [`dhash`] expects. One wider than tall, so
/// each row yields eight left-to-right comparisons.
pub const HASH_WIDTH: usize = 9;

/// Height of that buffer.
pub const HASH_HEIGHT: usize = 8;

/// A hit. Near-duplicate is not identity, so this holds rather than blocks.
pub const MATCH_DISTANCE: u32 = 10;

/// Pack a 9×8 greyscale image into 64 bits.
///
/// Each pixel is compared to its right neighbour; brighter sets the bit. The
/// comparison is relative, so overall brightness and contrast do not matter.
pub fn dhash(grey: &[u8]) -> u64 {
    if grey.len() < HASH_WIDTH * HASH_HEIGHT {
        return 0;
    }

    let mut hash = 0u64;
    let mut bit = 0;
    for row in 0..HASH_HEIGHT {
        let base = row * HASH_WIDTH;
        for col in 0..HASH_WIDTH - 1 {
            if grey[base + col] > grey[base + col + 1] {
                hash |= 1 << bit;
            }
            bit += 1;
        }
    }
    hash
}

/// How many bits differ. Brute-force XOR-popcount over tens of thousands of
/// blocklist entries is microseconds, so no index is needed.
pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Whether these are near-duplicates.
pub fn matches(a: u64, b: u64) -> bool {
    distance(a, b) <= MATCH_DISTANCE
}

/// The closest blocklist entry, if any is within [`MATCH_DISTANCE`].
pub fn find_match(hash: u64, blocklist: &[u64]) -> Option<(u64, u32)> {
    blocklist
        .iter()
        .map(|&known| (known, distance(hash, known)))
        .filter(|&(_, d)| d <= MATCH_DISTANCE)
        .min_by_key(|&(_, d)| d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gradient that gets darker left to right, so every comparison is set.
    fn descending() -> Vec<u8> {
        (0..HASH_HEIGHT)
            .flat_map(|_| (0..HASH_WIDTH).map(|c| (255 - c * 20) as u8))
            .collect()
    }

    fn flat(value: u8) -> Vec<u8> {
        vec![value; HASH_WIDTH * HASH_HEIGHT]
    }

    #[test]
    fn a_descending_gradient_sets_every_bit() {
        assert_eq!(dhash(&descending()), u64::MAX);
    }

    #[test]
    fn a_flat_image_sets_no_bits() {
        // Equal neighbours are not brighter, so nothing is set.
        assert_eq!(dhash(&flat(128)), 0);
        assert_eq!(dhash(&flat(0)), 0);
        assert_eq!(dhash(&flat(255)), 0);
    }

    #[test]
    fn brightness_does_not_change_the_hash() {
        // The comparison is relative, which is what survives a re-encode.
        let dim: Vec<u8> = descending().iter().map(|p| p / 2).collect();
        assert_eq!(dhash(&descending()), dhash(&dim));
    }

    #[test]
    fn a_small_change_moves_only_a_few_bits() {
        let mut tweaked = descending();
        tweaked[0] = 0;
        let d = distance(dhash(&descending()), dhash(&tweaked));
        assert!(d > 0 && d <= 2, "one pixel moved {d} bits");
    }

    #[test]
    fn unrelated_images_are_far_apart() {
        assert!(!matches(dhash(&descending()), dhash(&flat(128))));
        assert_eq!(distance(u64::MAX, 0), 64);
    }

    #[test]
    fn a_near_duplicate_is_within_the_threshold() {
        let original = dhash(&descending());
        let reencoded = original ^ 0b1010_1010; // four bits moved
        assert!(matches(original, reencoded));
        assert_eq!(distance(original, reencoded), 4);
    }

    #[test]
    fn eleven_bits_apart_is_not_a_match() {
        let a = 0u64;
        let b = (1u64 << 11) - 1;
        assert_eq!(distance(a, b), 11);
        assert!(!matches(a, b));
    }

    #[test]
    fn the_closest_blocklist_entry_wins() {
        let hash = 0u64;
        let far = 0xFFFF_FFFF_FFFF_FFFF;
        let near = 0b111;
        let nearer = 0b1;

        assert_eq!(find_match(hash, &[far, near, nearer]), Some((nearer, 1)));
        assert_eq!(find_match(hash, &[far]), None);
        assert_eq!(find_match(hash, &[]), None);
    }

    #[test]
    fn a_wrong_sized_buffer_returns_zero_rather_than_panicking() {
        assert_eq!(dhash(&[1, 2, 3]), 0);
    }

    #[test]
    fn the_hash_fits_a_signed_bigint_column() {
        // Postgres BIGINT is signed; the round trip must not lose the top bit.
        let hash = dhash(&descending());
        let stored = hash as i64;
        assert_eq!(stored as u64, hash);
    }
}
