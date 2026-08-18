//! Weighted round-robin across tenants within one lane.
//!
//! The property that matters: weight changes how *fast* a tenant is served,
//! never *whether*. A cheap plan is slower; it is never stuck behind someone
//! with 10,000 items queued.

use crate::model::TenantId;
use std::collections::HashMap;

/// A tenant with work waiting in this lane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// Whose work.
    pub tenant_id: TenantId,
    /// Copied from the plan at submit time. Premium 5.0, free 1.0.
    pub weight: f32,
    /// Slots this tenant may still take, from its plan limit.
    pub headroom: u32,
    /// Items queued in this lane.
    pub pending: u32,
}

impl Candidate {
    /// The most this tenant could take this tick.
    fn ceiling(&self) -> u32 {
        self.headroom.min(self.pending)
    }
}

/// How many items to pull for one tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    /// Whose work.
    pub tenant_id: TenantId,
    /// How many items to admit, always at least one.
    pub count: u32,
}

/// Proportional share with carried remainders.
///
/// Each tick a tenant earns `slots × weight / Σweights` credit and spends whole
/// credits; the fraction left over carries, so small weights accumulate a turn
/// instead of rounding to nothing forever. Slots left after that go to whoever
/// is owed most.
#[derive(Debug, Default)]
pub struct Fairness {
    credits: HashMap<TenantId, f32>,
}

/// Credit is clamped to this band, so nothing can be banked while idle and
/// nothing is owed forever.
const CREDIT_BAND: f32 = 1.0;

impl Fairness {
    /// A scheduler's fairness state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Split `slots` across `candidates`.
    ///
    /// Deterministic: the result depends only on the inputs and the carried
    /// credit, never on database row order.
    pub fn distribute(&mut self, slots: u32, candidates: &[Candidate]) -> Vec<Grant> {
        let mut eligible: Vec<Candidate> = candidates
            .iter()
            .copied()
            .filter(|c| c.ceiling() > 0 && c.weight > 0.0)
            .collect();
        self.forget_absent(&eligible);
        if slots == 0 || eligible.is_empty() {
            return Vec::new();
        }
        eligible.sort_unstable_by_key(|c| c.tenant_id);

        let mut taken: Vec<u32> = vec![0; eligible.len()];
        let mut remaining = slots;

        let total_weight: f32 = eligible.iter().map(|c| c.weight).sum();
        for (i, c) in eligible.iter().enumerate() {
            let share = slots as f32 * c.weight / total_weight;
            let credit = self.credits.entry(c.tenant_id).or_insert(0.0);
            *credit += share;

            let count = (credit.floor().max(0.0) as u32)
                .min(c.ceiling())
                .min(remaining);
            *credit -= count as f32;
            taken[i] = count;
            remaining -= count;
        }

        // Whatever flooring and plan limits left over goes to whoever is owed
        // most. Ties break on tenant id, so a tick is reproducible.
        while remaining > 0 {
            let next = (0..eligible.len())
                .filter(|&i| taken[i] < eligible[i].ceiling())
                .max_by(|&a, &b| {
                    let ca = self.credit_of(&eligible[a]);
                    let cb = self.credit_of(&eligible[b]);
                    // Most owed wins; a tie goes to the lower tenant id, which
                    // is the earlier index because eligible is sorted.
                    ca.total_cmp(&cb).then(std::cmp::Ordering::Greater)
                });

            let Some(i) = next else { break };
            *self.credits.entry(eligible[i].tenant_id).or_insert(0.0) -= 1.0;
            taken[i] += 1;
            remaining -= 1;
        }

        for c in &eligible {
            let credit = self.credits.entry(c.tenant_id).or_insert(0.0);
            *credit = credit.clamp(-CREDIT_BAND, CREDIT_BAND);
        }

        eligible
            .iter()
            .zip(&taken)
            .filter(|(_, count)| **count > 0)
            .map(|(c, count)| Grant {
                tenant_id: c.tenant_id,
                count: *count,
            })
            .collect()
    }

    fn credit_of(&self, c: &Candidate) -> f32 {
        self.credits.get(&c.tenant_id).copied().unwrap_or(0.0)
    }

    /// Drop credit for tenants with nothing queued, so the map tracks the
    /// active set rather than every tenant that ever submitted.
    fn forget_absent(&mut self, eligible: &[Candidate]) {
        if self.credits.len() <= eligible.len() {
            return;
        }
        let present: std::collections::HashSet<TenantId> =
            eligible.iter().map(|c| c.tenant_id).collect();
        self.credits.retain(|id, _| present.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn tenant(n: u8) -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = n;
        Uuid::from_bytes(bytes)
    }

    fn candidate(n: u8, weight: f32, pending: u32) -> Candidate {
        Candidate {
            tenant_id: tenant(n),
            weight,
            headroom: u32::MAX,
            pending,
        }
    }

    fn total(grants: &[Grant]) -> u32 {
        grants.iter().map(|g| g.count).sum()
    }

    fn count_for(grants: &[Grant], n: u8) -> u32 {
        grants
            .iter()
            .find(|g| g.tenant_id == tenant(n))
            .map_or(0, |g| g.count)
    }

    #[test]
    fn equal_weights_split_evenly() {
        let mut f = Fairness::new();
        let grants = f.distribute(10, &[candidate(1, 1.0, 100), candidate(2, 1.0, 100)]);
        assert_eq!(total(&grants), 10);
        assert_eq!(count_for(&grants, 1), 5);
        assert_eq!(count_for(&grants, 2), 5);
    }

    #[test]
    fn weight_five_gets_five_turns_per_weight_one() {
        let mut f = Fairness::new();
        let grants = f.distribute(60, &[candidate(1, 5.0, 1000), candidate(2, 1.0, 1000)]);
        assert_eq!(total(&grants), 60);
        assert_eq!(count_for(&grants, 1), 50);
        assert_eq!(count_for(&grants, 2), 10);
    }

    #[test]
    fn ten_thousand_items_cannot_take_over_from_a_single_item() {
        // The property the whole stage exists for.
        let mut f = Fairness::new();
        let grants = f.distribute(10, &[candidate(1, 1.0, 10_000), candidate(2, 1.0, 1)]);
        assert_eq!(count_for(&grants, 2), 1, "the small tenant was starved");
        assert_eq!(count_for(&grants, 1), 9);
    }

    #[test]
    fn a_free_plan_is_slower_than_premium_but_never_stuck() {
        // One slot per tick, premium queued deep. Free must still be served.
        let mut f = Fairness::new();
        let mut free_served = 0;
        for _ in 0..20 {
            let grants = f.distribute(1, &[candidate(1, 5.0, 1000), candidate(2, 1.0, 1000)]);
            free_served += count_for(&grants, 2);
        }
        assert!(free_served > 0, "weight decided whether, not how fast");
    }

    #[test]
    fn credit_carries_so_fractional_weights_are_not_lost() {
        // Weight 0.5 earns a turn every second pass rather than never.
        let mut f = Fairness::new();
        let mut served = 0;
        for _ in 0..6 {
            served += count_for(&f.distribute(1, &[candidate(1, 0.5, 100)]), 1);
        }
        assert!(served >= 2, "served {served} of 6 ticks at weight 0.5");
    }

    #[test]
    fn a_tenant_never_exceeds_its_plan_limit() {
        let mut f = Fairness::new();
        let mut c = candidate(1, 5.0, 1000);
        c.headroom = 3;
        let grants = f.distribute(100, &[c, candidate(2, 1.0, 1000)]);
        assert_eq!(count_for(&grants, 1), 3);
        assert_eq!(total(&grants), 100);
    }

    #[test]
    fn a_tenant_with_no_headroom_is_skipped_entirely() {
        let mut f = Fairness::new();
        let mut c = candidate(1, 5.0, 1000);
        c.headroom = 0;
        let grants = f.distribute(10, &[c, candidate(2, 1.0, 1000)]);
        assert_eq!(count_for(&grants, 1), 0);
        assert_eq!(count_for(&grants, 2), 10);
    }

    #[test]
    fn nothing_is_granted_beyond_what_is_queued() {
        let mut f = Fairness::new();
        let grants = f.distribute(100, &[candidate(1, 1.0, 3), candidate(2, 1.0, 2)]);
        assert_eq!(total(&grants), 5);
        assert_eq!(count_for(&grants, 1), 3);
        assert_eq!(count_for(&grants, 2), 2);
    }

    #[test]
    fn no_slots_or_no_candidates_grants_nothing() {
        let mut f = Fairness::new();
        assert!(f.distribute(0, &[candidate(1, 1.0, 10)]).is_empty());
        assert!(f.distribute(10, &[]).is_empty());
        assert!(f.distribute(10, &[candidate(1, 1.0, 0)]).is_empty());
    }

    #[test]
    fn a_returning_tenant_cannot_bank_credit_and_take_the_fleet() {
        let mut f = Fairness::new();
        // Idle for many ticks while another tenant is served.
        for _ in 0..50 {
            f.distribute(1, &[candidate(2, 1.0, 1000)]);
        }
        let grants = f.distribute(100, &[candidate(1, 1.0, 1000), candidate(2, 1.0, 1000)]);
        assert!(
            count_for(&grants, 1).abs_diff(count_for(&grants, 2)) <= 4,
            "banked credit skewed the split: {grants:?}"
        );
    }

    #[test]
    fn one_slot_a_tick_rotates_rather_than_always_favouring_the_lowest_id() {
        let mut f = Fairness::new();
        let mut served = [0u32; 3];
        for _ in 0..30 {
            let grants = f.distribute(
                1,
                &[
                    candidate(1, 1.0, 100),
                    candidate(2, 1.0, 100),
                    candidate(3, 1.0, 100),
                ],
            );
            for (i, n) in [1u8, 2, 3].into_iter().enumerate() {
                served[i] += count_for(&grants, n);
            }
        }
        assert!(
            served.iter().all(|&s| s > 0),
            "a tenant never led: {served:?}"
        );
    }

    #[test]
    fn the_result_does_not_depend_on_candidate_order() {
        let a = [
            candidate(1, 3.0, 100),
            candidate(2, 1.0, 100),
            candidate(3, 2.0, 100),
        ];
        let mut b = a;
        b.reverse();

        let mut f1 = Fairness::new();
        let mut f2 = Fairness::new();
        assert_eq!(f1.distribute(24, &a), f2.distribute(24, &b));
    }

    #[test]
    fn credit_for_departed_tenants_is_forgotten() {
        let mut f = Fairness::new();
        let many: Vec<Candidate> = (1..=20).map(|n| candidate(n, 1.0, 10)).collect();
        f.distribute(20, &many);
        f.distribute(1, &[candidate(1, 1.0, 10)]);
        assert!(f.credits.len() <= 2, "credits leaked: {}", f.credits.len());
    }

    #[test]
    fn a_long_run_stays_proportional_to_weight() {
        let mut f = Fairness::new();
        let mut served = [0u32; 2];
        for _ in 0..200 {
            let grants = f.distribute(3, &[candidate(1, 4.0, 10_000), candidate(2, 1.0, 10_000)]);
            served[0] += count_for(&grants, 1);
            served[1] += count_for(&grants, 2);
        }
        let ratio = f64::from(served[0]) / f64::from(served[1]);
        assert!(
            (3.8..=4.2).contains(&ratio),
            "ratio {ratio:.2}, served {served:?}"
        );
    }

    #[test]
    fn proportionality_survives_a_slot_supply_smaller_than_the_weights() {
        // Three slots a tick against weights 4:1 — the case where per-tick
        // rounding, not the algorithm, decides the answer unless credit carries.
        let mut f = Fairness::new();
        let mut served = [0u32; 2];
        for _ in 0..400 {
            let grants = f.distribute(3, &[candidate(1, 4.0, 10_000), candidate(2, 1.0, 10_000)]);
            served[0] += count_for(&grants, 1);
            served[1] += count_for(&grants, 2);
        }
        assert_eq!(served[0] + served[1], 1200, "slots were dropped");
        let ratio = f64::from(served[0]) / f64::from(served[1]);
        assert!(
            (3.8..=4.2).contains(&ratio),
            "ratio {ratio:.2}, served {served:?}"
        );
    }

    #[test]
    fn a_single_slot_a_tick_still_reaches_every_tenant() {
        // The starvation check at its harshest: one slot, ten tenants, one of
        // them queued 10,000 deep.
        let mut f = Fairness::new();
        let mut candidates: Vec<Candidate> = (2..=10).map(|n| candidate(n, 1.0, 5)).collect();
        candidates.push(candidate(1, 1.0, 10_000));

        let mut served = std::collections::HashMap::new();
        for _ in 0..200 {
            for g in f.distribute(1, &candidates) {
                *served.entry(g.tenant_id).or_insert(0u32) += g.count;
            }
        }
        for n in 1..=10u8 {
            assert!(
                served.get(&tenant(n)).copied().unwrap_or(0) > 0,
                "tenant {n} starved"
            );
        }
    }
}
