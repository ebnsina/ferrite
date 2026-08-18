//! How many slots each lane gets this tick.
//!
//! Owned hardware, so idle is waste: a lane may burst into anything the lanes
//! above it are not using, and `bulk` — being last — gets everything left.

use crate::model::Lane;
use serde::{Deserialize, Serialize};

/// The share of the fleet each lane is guaranteed when it has work, indexed by
/// [`Lane::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LaneShares(pub [f32; 3]);

impl Default for LaneShares {
    fn default() -> Self {
        Self([0.40, 0.50, 0.10])
    }
}

impl LaneShares {
    /// Slots `lane` is guaranteed out of `total`.
    pub fn guarantee(&self, lane: Lane, total: u32) -> u32 {
        (f64::from(total) * f64::from(self.0[lane as usize])).floor() as u32
    }
}

/// What the fleet is doing right now, per lane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LaneLoad {
    /// Items holding a slot in this lane.
    pub running: u32,
    /// Items queued in this lane.
    pub pending: u32,
}

/// One tick's view of the fleet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FleetState {
    /// Total slots across every worker.
    pub total_slots: u32,
    /// Per-lane load, indexed by [`Lane::ALL`].
    pub lanes: [LaneLoad; 3],
    /// Guarantees.
    pub shares: LaneShares,
}

impl FleetState {
    /// A fleet of `total_slots` with nothing running.
    pub fn empty(total_slots: u32) -> Self {
        Self {
            total_slots,
            lanes: [LaneLoad::default(); 3],
            shares: LaneShares::default(),
        }
    }

    /// Load for `lane`.
    pub fn lane(&self, lane: Lane) -> LaneLoad {
        self.lanes[lane as usize]
    }

    /// Mutable load for `lane`.
    pub fn lane_mut(&mut self, lane: Lane) -> &mut LaneLoad {
        &mut self.lanes[lane as usize]
    }

    /// Slots held across every lane.
    pub fn running(&self) -> u32 {
        self.lanes.iter().map(|l| l.running).sum()
    }

    /// Slots nobody is holding.
    pub fn free(&self) -> u32 {
        self.total_slots.saturating_sub(self.running())
    }
}

/// How many items each lane may admit this tick, indexed by [`Lane::ALL`].
///
/// Walks lanes highest-priority first. A lane takes what it wants from the free
/// pool, minus whatever the lanes below it need to reach their guarantees —
/// so `realtime` bursts into idle capacity but cannot starve `bulk` to zero.
pub fn lane_grants(state: &FleetState) -> [u32; 3] {
    let mut free = state.free();
    let mut grants = [0u32; 3];

    for (i, lane) in Lane::ALL.into_iter().enumerate() {
        let reserved_below: u32 = Lane::ALL[i + 1..]
            .iter()
            .map(|&lower| {
                let load = state.lane(lower);
                let shortfall = state
                    .shares
                    .guarantee(lower, state.total_slots)
                    .saturating_sub(load.running);
                shortfall.min(load.pending)
            })
            .sum();

        let budget = free.saturating_sub(reserved_below);
        let grant = budget.min(state.lane(lane).pending);
        grants[i] = grant;
        free -= grant;
    }
    grants
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(total: u32, load: [(u32, u32); 3]) -> FleetState {
        let mut s = FleetState::empty(total);
        for (i, (running, pending)) in load.into_iter().enumerate() {
            s.lanes[i] = LaneLoad { running, pending };
        }
        s
    }

    #[test]
    fn an_idle_fleet_admits_everything_that_fits() {
        let s = state(100, [(0, 10), (0, 10), (0, 10)]);
        assert_eq!(lane_grants(&s), [10, 10, 10]);
    }

    #[test]
    fn realtime_bursts_into_capacity_nobody_else_wants() {
        // 40% guarantee, but nothing else is queued, so it takes the fleet.
        let s = state(100, [(0, 500), (0, 0), (0, 0)]);
        assert_eq!(lane_grants(&s), [100, 0, 0]);
    }

    #[test]
    fn bulk_soaks_up_everything_left() {
        let s = state(100, [(0, 5), (0, 5), (0, 10_000)]);
        assert_eq!(lane_grants(&s), [5, 5, 90]);
    }

    #[test]
    fn realtime_cannot_starve_the_lanes_below_it() {
        // Realtime wants the whole fleet; standard and bulk have work, so their
        // guarantees (50 and 10) are held back.
        let s = state(100, [(0, 500), (0, 500), (0, 500)]);
        let grants = lane_grants(&s);
        assert_eq!(grants, [40, 50, 10]);
        assert_eq!(grants.iter().sum::<u32>(), 100);
    }

    #[test]
    fn a_lane_already_at_its_guarantee_reserves_nothing_more() {
        // Standard is running 50 already, so realtime may burst into the rest.
        let s = state(100, [(0, 500), (50, 500), (0, 0)]);
        assert_eq!(lane_grants(&s), [50, 0, 0]);
    }

    #[test]
    fn a_full_fleet_admits_nothing() {
        let s = state(64, [(30, 100), (30, 100), (4, 100)]);
        assert_eq!(lane_grants(&s), [0, 0, 0]);
    }

    #[test]
    fn grants_never_exceed_free_slots() {
        for total in [1u32, 7, 64, 1000] {
            for pending in [0u32, 1, 10_000] {
                let s = state(total, [(0, pending), (0, pending), (0, pending)]);
                assert!(lane_grants(&s).iter().sum::<u32>() <= s.free());
            }
        }
    }
}
