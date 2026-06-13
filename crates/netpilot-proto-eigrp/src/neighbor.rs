use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct EigrpNeighbor {
    pub router_id: String,
    pub interface: String,
    pub address: String,
    pub autonomous_system: u32,
    pub hold_time_secs: u16,
    pub hold_time_remaining_secs: u16,
    pub srtt_ms: u32,             // smooth round-trip time
    pub rto_ms: u32,              // retransmission timeout
    pub sequence_number: u32,     // last sequence sent
    pub last_hello_received: Option<OffsetDateTime>,
    pub is_up: bool,
}

impl EigrpNeighbor {
    pub fn new(router_id: &str, interface: &str, address: &str, asn: u32, hold_time_secs: u16) -> Self {
        Self {
            router_id: router_id.to_string(),
            interface: interface.to_string(),
            address: address.to_string(),
            autonomous_system: asn,
            hold_time_secs,
            hold_time_remaining_secs: hold_time_secs,
            srtt_ms: 1000,
            rto_ms: 5000,
            sequence_number: 0,
            last_hello_received: None,
            is_up: false,
        }
    }

    pub fn process_hello(&mut self) {
        self.hold_time_remaining_secs = self.hold_time_secs;
        self.last_hello_received = Some(OffsetDateTime::now_utc());
        self.is_up = true;
    }

    pub fn tick_hold_timer(&mut self) -> bool {
        if self.hold_time_remaining_secs > 0 {
            self.hold_time_remaining_secs -= 1;
        }
        if self.hold_time_remaining_secs == 0 && self.is_up {
            self.is_up = false;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NeighborTable {
    neighbors: std::collections::HashMap<String, EigrpNeighbor>, // keyed by router_id
}

impl NeighborTable {
    pub fn new() -> Self { Self::default() }

    pub fn get(&self, router_id: &str) -> Option<&EigrpNeighbor> {
        self.neighbors.get(router_id)
    }

    pub fn get_mut(&mut self, router_id: &str) -> Option<&mut EigrpNeighbor> {
        self.neighbors.get_mut(router_id)
    }

    pub fn upsert(&mut self, neighbor: EigrpNeighbor) {
        self.neighbors.insert(neighbor.router_id.clone(), neighbor);
    }

    pub fn up_neighbors(&self) -> impl Iterator<Item = &EigrpNeighbor> {
        self.neighbors.values().filter(|n| n.is_up)
    }

    pub fn len(&self) -> usize { self.neighbors.len() }

    pub fn tick_all(&mut self) -> Vec<String> {
        let mut down = Vec::new();
        for n in self.neighbors.values_mut() {
            if n.tick_hold_timer() {
                down.push(n.router_id.clone());
            }
        }
        down
    }
}
