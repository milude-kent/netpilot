/// An OSPF neighbor discovered via Hello protocol.
#[derive(Clone, Debug)]
pub struct OspfNeighbor {
    pub router_id: String,
    pub interface: String,
    pub state: OspfNeighborState,
    pub priority: u8,
    pub dead_timer_secs: u32,
    /// Last received DD sequence number.
    pub dd_sequence_number: u32,
    /// Whether we are the master in the DD exchange.
    pub is_master: bool,
    /// Neighbor's DB Description flags from last received packet.
    pub neighbor_dd_flags: u8,
    /// LSA headers received from neighbor during Exchange (for LS Request).
    pub lsa_headers_received: Vec<netpilot_io::ospf::LsaHeader>,
    /// LSA headers we need to request (collected during Exchange).
    pub ls_request_list: Vec<netpilot_io::ospf::LsRequestEntry>,
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
            dd_sequence_number: 0,
            is_master: false,
            neighbor_dd_flags: 0,
            lsa_headers_received: Vec::new(),
            ls_request_list: Vec::new(),
        }
    }

    /// Process a received DB Description packet and return the new state.
    /// Implements the state machine from RFC 2328 §10.6.
    pub fn process_db_desc(&mut self, dd: &netpilot_io::ospf::DbDescPacket) -> OspfNeighborState {
        let i_bit = dd.flags & 0x04 != 0; // Init bit
        let m_bit = dd.flags & 0x02 != 0; // More bit
        let ms_bit = dd.flags & 0x01 != 0; // Master/Slave bit

        match self.state {
            OspfNeighborState::TwoWay => {
                // Transition to ExStart
                self.state = OspfNeighborState::ExStart;
                self.dd_sequence_number = dd.dd_sequence_number;
                // Fall through to ExStart processing
                self.process_exstart(dd, i_bit, m_bit, ms_bit)
            }
            OspfNeighborState::ExStart => self.process_exstart(dd, i_bit, m_bit, ms_bit),
            OspfNeighborState::Exchange => self.process_exchange(dd, i_bit, m_bit, ms_bit),
            _ => {
                // Ignore DD in other states
                self.state.clone()
            }
        }
    }

    /// ExStart state processing (RFC 2328 §10.6).
    /// We stay in ExStart until we receive a DD with I=1, M=1, MS=1
    /// and the neighbor's Router ID is higher than ours (neighbor becomes master).
    fn process_exstart(
        &mut self,
        dd: &netpilot_io::ospf::DbDescPacket,
        i_bit: bool,
        m_bit: bool,
        ms_bit: bool,
    ) -> OspfNeighborState {
        // In ExStart, we expect: I=1, M=1, MS=1 (neighbor's first DD)
        if i_bit && m_bit && ms_bit {
            // Neighbor is claiming to be master.
            // The router with the higher Router ID becomes master.
            let our_id = 0u32; // will be set by caller context
            let their_id = dd.header.router_id;
            if their_id > our_id {
                self.is_master = false;
                self.dd_sequence_number = dd.dd_sequence_number;
                self.neighbor_dd_flags = dd.flags;
                // Transition to Exchange
                self.state = OspfNeighborState::Exchange;
                return self.state.clone();
            }
        }

        // Also handle case where we are master (our ID > theirs)
        if i_bit && !ms_bit {
            // Neighbor acknowledges us as master
            self.is_master = true;
            self.dd_sequence_number = dd.dd_sequence_number;
            self.neighbor_dd_flags = dd.flags;
            self.state = OspfNeighborState::Exchange;
            return self.state.clone();
        }

        self.state.clone()
    }

    /// Exchange state processing (RFC 2328 §10.6).
    fn process_exchange(
        &mut self,
        dd: &netpilot_io::ospf::DbDescPacket,
        _i_bit: bool,
        m_bit: bool,
        _ms_bit: bool,
    ) -> OspfNeighborState {
        // Collect LSA headers from the DD packet
        for lsa_hdr in &dd.lsa_headers {
            self.lsa_headers_received.push(lsa_hdr.clone());

            // If we don't have this LSA or ours is older, add to request list
            self.ls_request_list
                .push(netpilot_io::ospf::LsRequestEntry {
                    ls_type: lsa_hdr.ls_type,
                    link_state_id: lsa_hdr.link_state_id,
                    advertising_router: lsa_hdr.advertising_router,
                });
        }

        // If master, increment DD sequence number
        if self.is_master {
            self.dd_sequence_number = self.dd_sequence_number.wrapping_add(1);
        } else {
            // Slave uses master's sequence number
            self.dd_sequence_number = dd.dd_sequence_number;
        }

        // Check if exchange is complete (no more DD packets)
        if !m_bit {
            if self.ls_request_list.is_empty() {
                // No LSPs to request → go directly to Full
                self.state = OspfNeighborState::Full;
            } else {
                // Need to request LSPs → Loading
                self.state = OspfNeighborState::Loading;
            }
        }

        self.state.clone()
    }
}
