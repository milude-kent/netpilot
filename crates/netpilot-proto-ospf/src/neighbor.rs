/// An OSPF neighbor discovered via Hello protocol.
#[derive(Clone, Debug)]
pub struct OspfNeighbor {
    pub router_id: String,
    pub interface: String,
    pub state: OspfNeighborState,
    pub priority: u8,
    pub dead_timer_secs: u32,
}

/// OSPF neighbor adjacency states (RFC 2328 Section 10.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OspfNeighborState {
    Down,
    Init,
    TwoWay,
    ExStart,
    Exchange,
    Loading,
    Full,
}

impl OspfNeighbor {
    pub fn new(router_id: &str, interface: &str) -> Self {
        Self {
            router_id: router_id.to_string(),
            interface: interface.to_string(),
            state: OspfNeighborState::Down,
            priority: 0,
            dead_timer_secs: 40,
        }
    }
}
