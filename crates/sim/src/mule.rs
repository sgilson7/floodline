//! The cart that carries a city's trade to another city and brings gold back.
//!
//! A mule is its own thing and not a citizen wearing a hat. The plan asks for
//! that and it is right: a citizen has hunger, rest, a home, a job, a crowd
//! around it and an errand it can abandon, and a mule needs none of those. It
//! has a position, somewhere it is going, what it is carrying and one bit for
//! whether it is on its way out or on its way home. Making it a citizen would
//! have meant six rules that do not apply to it and one that does.
//!
//! What it does get for free is the two things that matter on the road: it
//! walks the same flow fields everybody else does, so it finds its way round
//! the river the same way, and it moves at road speed on a road because
//! `Citizen::speed` is not the only thing that reads `carries_traffic`.

use crate::balance::*;
use crate::building::{BuildingId, Good, Goods};
use crate::citizen::PlayerId;
use crate::fx::V2;
use crate::nav::Dest;
use serde::{Deserialize, Serialize};

/// Index into `World::mules`. Never reused and never removed, like every other
/// id here: a retired mule stays in the vector so an id means one thing for a
/// whole run.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MuleId(pub u16);

/// Which way round the trip a mule is on.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Leg {
    /// Loaded, on the way to the other city.
    Out,
    /// Paid, on the way back to its own post.
    Home,
    /// Loaded but with nowhere to go: no other city it can reach. It waits at
    /// its post and the panel says so, which is the whole reason this is a
    /// state and not a mule standing still for reasons nobody can see.
    Stuck,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Mule {
    pub id: MuleId,
    pub owner: PlayerId,
    /// The post that sent it. When that stops standing, so does the mule.
    pub post: BuildingId,
    pub pos: V2,
    pub carrying: Goods,
    pub leg: Leg,
    /// Where it is walking, or nothing when it has arrived and is waiting for
    /// the next tick to turn it round.
    pub dest: Option<Dest>,
    /// A mule whose trader was unassigned, or whose post was taken by the
    /// flood. Kept in the vector; it does not move and is not drawn.
    pub retired: bool,
}

impl Mule {
    pub fn new(id: MuleId, owner: PlayerId, post: BuildingId, at: V2) -> Mule {
        Mule {
            id,
            owner,
            post,
            pos: at,
            carrying: Goods::NONE,
            leg: Leg::Out,
            dest: None,
            retired: false,
        }
    }

    pub fn alive(&self) -> bool {
        !self.retired
    }

    /// What a loaded mule takes to the other city.
    pub fn load() -> Goods {
        Goods::wood(MULE_LOAD)
    }

    /// What it is paid for it.
    pub fn pay() -> Goods {
        Goods::gold(MULE_PAY)
    }

    /// Drop what it is carrying. Design §6 says a hauler caught by the flood
    /// loses its cargo; a mule is a hauler with four legs.
    pub fn spill(&mut self) {
        self.carrying = Goods::NONE;
        // Empty and homebound: there is nothing to deliver any more, and a
        // mule that kept walking to the other city to hand over nothing would
        // be a bug you could watch.
        self.leg = Leg::Home;
        self.dest = None;
    }

    pub fn carrying_any(&self) -> bool {
        Good::ALL.into_iter().any(|g| self.carrying.get(g) > 0)
    }
}
