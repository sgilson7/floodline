//! Families: who shares a cottage, and where the next generation comes from.
//!
//! Design §3.2 sketches it in one line — "two adult citizens sharing a cottage
//! for a day become a household" — and the original milestone list deferred it.
//! This is that line, and the two rules that hang off it.
//!
//! **Being fed is the whole gate.** A hungry city does not grow, which is what
//! makes the granary the thing that decides the *size* of a village rather than
//! only whether it survives. And **no nursery, no children**: a child is born
//! into one and takes a place there, so a nursery is a building a player
//! chooses to put up rather than a rule that happens to them.

use crate::balance::*;
use crate::building::BuildingId;
use crate::citizen::{CitizenId, PlayerId};
use serde::{Deserialize, Serialize};

/// Index into `World::households`. Never reused, like every other id here.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct HouseholdId(pub u16);

/// Two adults, a cottage, and what has come of it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Household {
    pub id: HouseholdId,
    pub owner: PlayerId,
    /// The two of them, in id order — so a household formed on two machines is
    /// the same household, written the same way.
    pub members: [CitizenId; 2],
    pub cottage: BuildingId,
    /// Ticks the two of them have shared this cottage, fed. Reset by a hungry
    /// day, so a city that lets its larder run down loses the progress rather
    /// than pausing it.
    pub together: u32,
    /// Ticks toward the next child, once they are a household.
    pub toward_child: u32,
    pub children: Vec<CitizenId>,
    /// A household whose cottage is gone, or one of whose members has died.
    /// Kept in the vector; the children stay children of nobody, which is
    /// sad and is also exactly what the flood does.
    pub ended: bool,
}

impl Household {
    pub fn new(
        id: HouseholdId,
        owner: PlayerId,
        members: [CitizenId; 2],
        cottage: BuildingId,
    ) -> Household {
        Household {
            id,
            owner,
            members,
            cottage,
            together: 0,
            toward_child: 0,
            children: Vec::new(),
            ended: false,
        }
    }

    pub fn alive(&self) -> bool {
        !self.ended
    }

    /// Whether the two of them have shared the cottage long enough to be a
    /// household rather than two people who happen to sleep in one place.
    pub fn settled(&self) -> bool {
        self.together >= TICKS_PER_DAY
    }

    /// How close the next child is, from nought to a hundred. The households
    /// tab draws this; without it "a household with a fed larder produces a
    /// child on a timer" is a thing that happens *to* a player rather than
    /// something they can watch coming.
    pub fn expecting(&self) -> u32 {
        if !self.settled() {
            return 0;
        }
        (self.toward_child * 100 / CHILD_TICKS.max(1)).min(100)
    }
}
