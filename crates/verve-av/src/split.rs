//! Where to cut (pipeline step 5). Where distributed conversion goes wrong.
//!
//! Pure, and the plan is data so a retried chunk reproduces identical
//! boundaries (split rule 6).

use serde::{Deserialize, Serialize};

/// The industry norm: shorter means more parallelism and worse compression.
pub const TARGET_CHUNK_MS: u64 = 10_000;

/// Below this the overheads cost more than the parallelism gains (rule 7).
pub const MIN_SPLITTABLE_MS: u64 = 120_000;

/// One piece of the timeline, cut at keyframes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    /// Position in the sequence. Joins depend on this order.
    pub index: u32,
    /// First frame's presentation time.
    pub start_ms: u64,
    /// One past the last frame's presentation time.
    pub end_ms: u64,
}

impl Chunk {
    /// How much timeline this chunk covers.
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

/// Why a source ended up as a single chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Whole {
    /// Under [`MIN_SPLITTABLE_MS`].
    TooShort,
    /// No keyframe index, so cuts cannot be placed (rule 1).
    NoKeyframes,
    /// Keyframes too far apart to make chunks near the target.
    KeyframesTooSparse,
}

/// The saved plan. Written down, not recomputed, so a retry cuts identically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitPlan {
    /// Chunks in order, covering the whole timeline with no gaps.
    pub chunks: Vec<Chunk>,
    /// Set when the source was left whole, with the reason.
    pub whole: Option<Whole>,
}

impl SplitPlan {
    /// Whether this source fans out across machines.
    pub fn is_split(&self) -> bool {
        self.whole.is_none() && self.chunks.len() > 1
    }
}

/// Plan the cuts.
///
/// `keyframes_ms` comes from the probe; only these are legal boundaries, or a
/// chunk cannot be decoded on its own (rule 1).
pub fn plan(keyframes_ms: &[u64], duration_ms: u64, target_ms: u64) -> SplitPlan {
    let whole = |reason| SplitPlan {
        chunks: vec![Chunk {
            index: 0,
            start_ms: 0,
            end_ms: duration_ms,
        }],
        whole: Some(reason),
    };

    if duration_ms < MIN_SPLITTABLE_MS {
        return whole(Whole::TooShort);
    }
    if keyframes_ms.is_empty() {
        return whole(Whole::NoKeyframes);
    }

    // Walk the keyframes, cutting at the first one at or past each target.
    let target = target_ms.max(1);
    let mut boundaries = vec![0u64];
    let mut last = 0u64;
    for &k in keyframes_ms {
        if k > last && k - last >= target {
            boundaries.push(k);
            last = k;
        }
    }

    // One boundary means every keyframe sat inside a single target window.
    if boundaries.len() < 2 {
        return whole(Whole::KeyframesTooSparse);
    }

    let chunks = boundaries
        .iter()
        .enumerate()
        .map(|(i, &start)| Chunk {
            index: i as u32,
            start_ms: start,
            end_ms: boundaries.get(i + 1).copied().unwrap_or(duration_ms),
        })
        .filter(|c| c.end_ms > c.start_ms)
        .collect::<Vec<_>>();

    SplitPlan {
        chunks,
        whole: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keyframes every `every_ms` up to `duration_ms`.
    fn keyframes(every_ms: u64, duration_ms: u64) -> Vec<u64> {
        (0..)
            .map(|i| i * every_ms)
            .take_while(|&k| k < duration_ms)
            .collect()
    }

    fn covers(plan: &SplitPlan, duration_ms: u64) {
        assert_eq!(
            plan.chunks[0].start_ms, 0,
            "timeline does not start at zero"
        );
        assert_eq!(
            plan.chunks.last().unwrap().end_ms,
            duration_ms,
            "timeline does not reach the end"
        );
        for pair in plan.chunks.windows(2) {
            assert_eq!(
                pair[0].end_ms, pair[1].start_ms,
                "gap or overlap at {pair:?}"
            );
            assert_eq!(pair[1].index, pair[0].index + 1, "indexes out of order");
        }
    }

    #[test]
    fn a_ten_minute_video_cuts_into_roughly_sixty_chunks() {
        let d = 600_000;
        let p = plan(&keyframes(2_000, d), d, TARGET_CHUNK_MS);
        assert!(p.is_split());
        assert_eq!(p.chunks.len(), 60);
        covers(&p, d);
    }

    #[test]
    fn every_cut_lands_on_a_keyframe() {
        let d = 600_000;
        let keys = keyframes(2_000, d);
        let p = plan(&keys, d, TARGET_CHUNK_MS);
        for c in &p.chunks {
            assert!(
                keys.contains(&c.start_ms),
                "chunk starts at {} — not a keyframe",
                c.start_ms
            );
        }
    }

    #[test]
    fn the_plan_covers_the_timeline_with_no_gaps() {
        for every in [1_000u64, 2_000, 4_000, 5_000] {
            let d = 900_000;
            covers(&plan(&keyframes(every, d), d, TARGET_CHUNK_MS), d);
        }
    }

    #[test]
    fn chunks_are_never_shorter_than_the_target() {
        let d = 600_000;
        let p = plan(&keyframes(2_000, d), d, TARGET_CHUNK_MS);
        // The last chunk is whatever is left, so it is exempt.
        for c in &p.chunks[..p.chunks.len() - 1] {
            assert!(c.duration_ms() >= TARGET_CHUNK_MS, "{c:?} is under target");
        }
    }

    #[test]
    fn something_under_two_minutes_is_left_whole() {
        let d = 90_000;
        let p = plan(&keyframes(2_000, d), d, TARGET_CHUNK_MS);
        assert_eq!(p.whole, Some(Whole::TooShort));
        assert!(!p.is_split());
        covers(&p, d);
    }

    #[test]
    fn a_source_with_no_keyframe_index_is_left_whole() {
        let d = 600_000;
        let p = plan(&[], d, TARGET_CHUNK_MS);
        assert_eq!(p.whole, Some(Whole::NoKeyframes));
        covers(&p, d);
    }

    #[test]
    fn keyframes_too_sparse_to_cut_leave_the_source_whole() {
        // One keyframe at the start and nothing else for ten minutes.
        let d = 600_000;
        let p = plan(&[0], d, TARGET_CHUNK_MS);
        assert_eq!(p.whole, Some(Whole::KeyframesTooSparse));
        covers(&p, d);
    }

    #[test]
    fn sparse_keyframes_give_fewer_longer_chunks_rather_than_bad_ones() {
        // Keyframes every 30s against a 10s target: cuts land where they can.
        let d = 600_000;
        let p = plan(&keyframes(30_000, d), d, TARGET_CHUNK_MS);
        assert!(p.is_split());
        assert_eq!(p.chunks.len(), 20);
        covers(&p, d);
        assert!(p.chunks.iter().all(|c| c.duration_ms() >= 30_000));
    }

    #[test]
    fn a_source_that_starts_late_still_produces_a_covering_plan() {
        // First keyframe at 400ms, as a stream with a start offset would be.
        let d = 600_000;
        let mut keys = keyframes(2_000, d);
        keys.remove(0);
        let p = plan(&keys, d, TARGET_CHUNK_MS);
        covers(&p, d);
    }

    #[test]
    fn the_plan_is_reproducible_so_a_retry_cuts_identically() {
        let d = 600_000;
        let keys = keyframes(2_000, d);
        assert_eq!(
            plan(&keys, d, TARGET_CHUNK_MS),
            plan(&keys, d, TARGET_CHUNK_MS),
            "split rule 6: a retried chunk must reproduce its boundaries"
        );
    }

    #[test]
    fn the_plan_survives_a_json_round_trip() {
        let d = 600_000;
        let p = plan(&keyframes(2_000, d), d, TARGET_CHUNK_MS);
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<SplitPlan>(&json).unwrap(), p);
    }

    #[test]
    fn a_zero_target_does_not_hang_or_divide_by_zero() {
        let d = 600_000;
        let p = plan(&keyframes(2_000, d), d, 0);
        covers(&p, d);
    }
}
