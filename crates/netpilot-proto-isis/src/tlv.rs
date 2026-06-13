/// IS-IS TLV type codes.
pub mod tlv_types {
    pub const AREA_ADDRESSES: u8 = 1;
    pub const IS_NEIGHBORS: u8 = 2;
    pub const LAN_ID: u8 = 6;
    pub const PROTOCOLS_SUPPORTED: u8 = 129;
    pub const IP_INTERNAL_REACHABILITY: u8 = 128;
    pub const IP_EXTERNAL_REACHABILITY: u8 = 130;
    pub const EXTENDED_IS_REACHABILITY: u8 = 22;
    pub const IPV6_REACHABILITY: u8 = 236;
    pub const HOSTNAME: u8 = 137;
    pub const ROUTER_CAPABILITY: u8 = 242;
    pub const SR_CAPABILITY: u8 = 242; // sub-TLV of Router Capability
    pub const PREFIX_SID: u8 = 235;
    pub const ADJACENCY_SID: u8 = 240;
    pub const DYNAMIC_HOSTNAME: u8 = 137;
}

/// All supported IS-IS TLV variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IsisTlv {
    AreaAddresses(Vec<String>), // TLV 1
    IsNeighbors(Vec<String>),   // TLV 2
    LanId {
        system_id: String,
        pseudonode_id: u8,
    }, // TLV 6
    ProtocolsSupported(Vec<u8>), // TLV 129
    IpInternalReachability(Vec<IpReachEntry>), // TLV 128
    IpExternalReachability(Vec<IpReachEntry>), // TLV 130
    ExtendedIsReachability(Vec<ExtendedNeighbor>), // TLV 22
    Ipv6Reachability(Vec<Ipv6ReachEntry>), // TLV 236
    Hostname(String),           // TLV 137
    SrCapability {
        flags: u8,
        srgb_start: u32,
        srgb_end: u32,
    }, // TLV 242 sub-TLV
    PrefixSid(PrefixSidEntry),  // TLV 235
    AdjacencySid(AdjacencySidEntry), // TLV 240
    Unknown {
        type_code: u8,
        value: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpReachEntry {
    pub prefix: String,
    pub metric: u32,
    pub up_down: bool, // true = up (toward L2), false = down
    pub sub_tlv: bool, // has sub-TLVs
    pub prefix_len: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ipv6ReachEntry {
    pub prefix: String,
    pub metric: u32,
    pub up_down: bool,
    pub prefix_len: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtendedNeighbor {
    pub system_id: String,
    pub metric: u32,
    pub pseudonode_id: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixSidEntry {
    pub prefix: String,
    pub sid_index: u32,
    pub flags: u8,
    pub algorithm: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdjacencySidEntry {
    pub neighbor_system_id: String,
    pub sid_value: u32,
    pub flags: u8,
    pub weight: u8,
}

/// Parse raw bytes into a vector of TLVs.
/// TLVs are type-length-value encoded: [type:1][length:1][value:length]
pub fn parse_tlvs(data: &[u8]) -> Vec<IsisTlv> {
    let mut tlvs = Vec::new();
    let mut offset = 0;

    while offset + 1 < data.len() {
        let type_code = data[offset];
        let length = data[offset + 1] as usize;
        offset += 2;

        if offset + length > data.len() {
            break; // truncated TLV
        }

        let value = &data[offset..offset + length];
        let tlv = parse_single_tlv(type_code, value);
        tlvs.push(tlv);
        offset += length;
    }

    tlvs
}

fn parse_single_tlv(type_code: u8, value: &[u8]) -> IsisTlv {
    match type_code {
        tlv_types::AREA_ADDRESSES => {
            // Each area address is length-prefixed: [len][bytes...]
            let mut addrs = Vec::new();
            let mut off = 0;
            while off < value.len() {
                let len = value[off] as usize;
                off += 1;
                if off + len <= value.len() {
                    addrs.push(hex_encode(&value[off..off + len]));
                    off += len;
                } else {
                    break;
                }
            }
            IsisTlv::AreaAddresses(addrs)
        }
        tlv_types::IS_NEIGHBORS => {
            // Each neighbor is 6 bytes (system ID) + optional pseudonode
            let mut neighbors = Vec::new();
            for chunk in value.chunks(6) {
                if chunk.len() == 6 {
                    neighbors.push(format!(
                        "{:02x}{:02x}.{:02x}{:02x}.{:02x}{:02x}",
                        chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5]
                    ));
                }
            }
            IsisTlv::IsNeighbors(neighbors)
        }
        tlv_types::EXTENDED_IS_REACHABILITY => {
            let mut neighbors = Vec::new();
            for chunk in value.chunks(11) {
                if chunk.len() >= 11 {
                    neighbors.push(ExtendedNeighbor {
                        system_id: format!(
                            "{:02x}{:02x}.{:02x}{:02x}.{:02x}{:02x}",
                            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5]
                        ),
                        metric: u32::from_be_bytes([0, chunk[6], chunk[7], chunk[8]]),
                        pseudonode_id: chunk[10] & 0x3F,
                    });
                }
            }
            IsisTlv::ExtendedIsReachability(neighbors)
        }
        tlv_types::HOSTNAME => IsisTlv::Hostname(String::from_utf8_lossy(value).to_string()),
        tlv_types::SR_CAPABILITY => {
            // Simplified: expect 2 bytes flags + 6 bytes SRGB
            let flags = if value.len() >= 2 {
                u16::from_be_bytes([value[0], value[1]]) as u8
            } else {
                0
            };
            IsisTlv::SrCapability {
                flags,
                srgb_start: 16000,
                srgb_end: 24000,
            }
        }
        _ => IsisTlv::Unknown {
            type_code,
            value: value.to_vec(),
        },
    }
}

/// Build a byte vector from a slice of TLVs.
pub fn build_tlvs(tlvs: &[IsisTlv]) -> Vec<u8> {
    let mut buf = Vec::new();
    for tlv in tlvs {
        let (type_code, value) = encode_tlv(tlv);
        buf.push(type_code);
        buf.push(value.len() as u8);
        buf.extend_from_slice(&value);
    }
    buf
}

fn encode_tlv(tlv: &IsisTlv) -> (u8, Vec<u8>) {
    match tlv {
        IsisTlv::AreaAddresses(addrs) => {
            let mut v = Vec::new();
            for addr in addrs {
                let bytes = hex_decode(addr).unwrap_or_default();
                v.push(bytes.len() as u8);
                v.extend_from_slice(&bytes);
            }
            (tlv_types::AREA_ADDRESSES, v)
        }
        IsisTlv::IsNeighbors(neighbors) => {
            let mut v = Vec::new();
            for n in neighbors {
                if let Some(bytes) = hex_decode(n) {
                    v.extend_from_slice(&bytes);
                }
            }
            (tlv_types::IS_NEIGHBORS, v)
        }
        IsisTlv::ExtendedIsReachability(neighbors) => {
            let mut v = Vec::new();
            for n in neighbors {
                if let Some(sys_bytes) = hex_decode(&n.system_id) {
                    v.extend_from_slice(&sys_bytes);
                }
                let metric_bytes = n.metric.to_be_bytes();
                v.extend_from_slice(&metric_bytes[1..]); // 3 bytes
                v.push(n.pseudonode_id & 0x3F);
            }
            (tlv_types::EXTENDED_IS_REACHABILITY, v)
        }
        IsisTlv::Hostname(name) => (tlv_types::HOSTNAME, name.as_bytes().to_vec()),
        IsisTlv::SrCapability {
            flags,
            srgb_start,
            srgb_end,
        } => {
            let mut v = Vec::new();
            v.push(*flags);
            v.push(0); // reserved
            v.extend_from_slice(&u16::to_be_bytes((*srgb_start >> 16) as u16));
            v.extend_from_slice(&u16::to_be_bytes(*srgb_start as u16));
            v.extend_from_slice(&u16::to_be_bytes((*srgb_end >> 16) as u16));
            v.extend_from_slice(&u16::to_be_bytes(*srgb_end as u16));
            (tlv_types::SR_CAPABILITY, v)
        }
        IsisTlv::PrefixSid(entry) => {
            let mut v = Vec::new();
            v.push(entry.flags);
            v.push(entry.algorithm);
            v.extend_from_slice(&entry.sid_index.to_be_bytes());
            (tlv_types::PREFIX_SID, v)
        }
        IsisTlv::AdjacencySid(entry) => {
            let mut v = Vec::new();
            v.push(entry.flags);
            v.push(entry.weight);
            if let Some(bytes) = hex_decode(&entry.neighbor_system_id) {
                v.extend_from_slice(&bytes);
            }
            v.extend_from_slice(&entry.sid_value.to_be_bytes());
            (tlv_types::ADJACENCY_SID, v)
        }
        _ => (0, vec![]),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.replace(['.', '-'], "");
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
