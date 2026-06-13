use crate::config::IsisLevel;
use crate::packet::IihPacket;
use time::OffsetDateTime;

/// Three-state adjacency machine: Down → Init → Up → Down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdjacencyState {
    Down,
    Init,
    Up,
}

#[derive(Clone, Debug)]
pub struct Adjacency {
    pub neighbor_system_id: String,
    pub interface: String,
    pub level: IsisLevel,
    pub state: AdjacencyState,
    pub neighbor_priority: u8,
    pub local_system_id: String,
    pub holding_timer_remaining_secs: u32,
    pub last_hello_received: Option<OffsetDateTime>,
}

impl Adjacency {
    pub fn new(neighbor_system_id: &str, interface: &str, level: IsisLevel, local_system_id: &str, holding_time_secs: u32) -> Self {
        Self {
            neighbor_system_id: neighbor_system_id.to_string(),
            interface: interface.to_string(),
            level,
            state: AdjacencyState::Down,
            neighbor_priority: 0,
            local_system_id: local_system_id.to_string(),
            holding_timer_remaining_secs: holding_time_secs,
            last_hello_received: None,
        }
    }

    /// Process an incoming IIH packet. Returns the new state.
    pub fn process_hello(&mut self, iih: &IihPacket) -> AdjacencyState {
        self.holding_timer_remaining_secs = iih.holding_time_secs as u32;
        self.last_hello_received = Some(OffsetDateTime::now_utc());
        self.neighbor_priority = iih.priority;

        match self.state {
            AdjacencyState::Down => {
                self.state = AdjacencyState::Init;
            }
            AdjacencyState::Init => {
                // Transition to Up if we see ourselves in neighbor's IIH
                if iih.neighbors.iter().any(|n| n == &self.local_system_id) {
                    self.state = AdjacencyState::Up;
                }
            }
            AdjacencyState::Up => {
                // Stay Up — holding timer was already refreshed
            }
        }
        self.state.clone()
    }

    /// Decrement holding timer. Returns true if expired (needs Down transition).
    pub fn tick_holding_timer(&mut self) -> bool {
        if self.holding_timer_remaining_secs > 0 {
            self.holding_timer_remaining_secs -= 1;
        }
        if self.holding_timer_remaining_secs == 0 && self.state != AdjacencyState::Down {
            self.state = AdjacencyState::Down;
            true
        } else {
            false
        }
    }

    pub fn is_up(&self) -> bool {
        self.state == AdjacencyState::Up
    }

    pub fn holding_timer_expired(&self) -> bool {
        self.holding_timer_remaining_secs == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::IihPacket;

    fn make_iih(with_local_id: bool) -> IihPacket {
        let mut neighbors = vec!["1920.0000.0002".to_string()];
        if with_local_id {
            neighbors.push("1920.0000.0001".to_string());
        }
        IihPacket {
            circuit_type: 3,
            source_id: "1920.0000.0002".to_string(),
            holding_time_secs: 30,
            pdu_length: 100,
            priority: 64,
            lan_id: None,
            neighbors,
            tlvs: vec![],
        }
    }

    #[test]
    fn adjacency_down_to_init_on_hello() {
        let mut adj = Adjacency::new("1920.0000.0002", "eth0", IsisLevel::Level2, "1920.0000.0001", 30);
        assert_eq!(adj.state, AdjacencyState::Down);
        let state = adj.process_hello(&make_iih(false));
        assert_eq!(state, AdjacencyState::Init);
    }

    #[test]
    fn adjacency_init_to_up_when_seen() {
        let mut adj = Adjacency::new("1920.0000.0002", "eth0", IsisLevel::Level2, "1920.0000.0001", 30);
        adj.process_hello(&make_iih(false)); // Down → Init
        let state = adj.process_hello(&make_iih(true)); // Init → Up
        assert_eq!(state, AdjacencyState::Up);
    }

    #[test]
    fn adjacency_expires_on_holding_timer() {
        let mut adj = Adjacency::new("1920.0000.0002", "eth0", IsisLevel::Level2, "1920.0000.0001", 1);
        adj.process_hello(&make_iih(false)); // Down → Init
        adj.process_hello(&make_iih(true));  // Init → Up
        assert_eq!(adj.state, AdjacencyState::Up);
        // Override timer to 1 — IIH packets refresh it to 30
        adj.holding_timer_remaining_secs = 1;
        let expired = adj.tick_holding_timer();
        assert!(expired);
        assert_eq!(adj.state, AdjacencyState::Down);
    }
}
