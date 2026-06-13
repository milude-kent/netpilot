/// EIGRP TLV type codes
pub mod tlv_types {
    pub const PARAMETER: u16 = 0x0001;
    pub const SEQUENCE: u16 = 0x0003;
    pub const IP_INTERNAL: u16 = 0x0102;
    pub const IP_EXTERNAL: u16 = 0x0103;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EigrpTlv {
    Parameter(Params),
    Sequence(SequenceTlv),
    IpInternal(IpInternalRoute),
    IpExternal(IpExternalRoute),
    Unknown { type_code: u16, value: Vec<u8> },
}

/// EIGRP Parameters TLV (sent in Hello and first Update).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    pub k_values: crate::config::KValues,
    pub hold_time_secs: u16,
}

/// Sequence TLV — carries retransmission info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceTlv {
    pub address_family: u16,
    pub sequence_numbers: Vec<u32>,
}

/// Internal EIGRP route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpInternalRoute {
    pub next_hop: String,
    pub prefix: String,
    pub prefix_length: u8,
    pub metric: EigrpMetric,
    pub mtu: u32,
    pub hop_count: u8,
}

/// External EIGRP route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpExternalRoute {
    pub next_hop: String,
    pub prefix: String,
    pub prefix_length: u8,
    pub metric: EigrpMetric,
    pub originating_router_id: String,
    pub originating_as: u32,
    pub mtu: u32,
    pub hop_count: u8,
}

/// Composite EIGRP metric (32-bit).
/// Formula: 256 * ((K1*BW + K2*BW/(256-load) + K3*delay) * K5/(reliability+K4))
/// With defaults K1=1,K3=1: 256 * (BW + delay)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EigrpMetric {
    pub bandwidth: u32,   // scaled: 10^7 / min_bandwidth_kbps
    pub delay: u32,        // scaled: sum_delay_tens_of_microseconds / 10
    pub reliability: u8,   // 255 = 100%
    pub load: u8,          // 255 = 100%
    pub mtu: u32,
    pub hop_count: u8,
}

impl EigrpMetric {
    /// Compute composite metric using default K-values (K1=1, K3=1).
    pub fn composite(&self) -> u32 {
        256u32.saturating_mul(self.bandwidth.saturating_add(self.delay))
    }

    pub fn infinity() -> Self {
        Self { bandwidth: u32::MAX, delay: u32::MAX, ..Default::default() }
    }
}
