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
            let mut off = 0;
            while off + 11 <= value.len() {
                let system_id = format!(
                    "{:02x}{:02x}.{:02x}{:02x}.{:02x}{:02x}",
                    value[off],
                    value[off + 1],
                    value[off + 2],
                    value[off + 3],
                    value[off + 4],
                    value[off + 5]
                );
                let pseudonode_id = value[off + 6];
                let metric =
                    u32::from_be_bytes([0, value[off + 7], value[off + 8], value[off + 9]]);
                let sub_tlv_len = value[off + 10] as usize;
                neighbors.push(ExtendedNeighbor {
                    system_id,
                    metric,
                    pseudonode_id,
                });
                off += 11 + sub_tlv_len;
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
        tlv_types::IP_INTERNAL_REACHABILITY => {
            let mut entries = Vec::new();
            let mut off = 0;
            while off + 8 <= value.len() {
                let metric_byte = value[off];
                let metric = (metric_byte & 0x7F) as u32;
                let up_down = metric_byte & 0x80 != 0;
                let ip_bytes = &value[off + 4..off + 8];
                let prefix = format!(
                    "{}.{}.{}.{}/32",
                    ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
                );
                entries.push(IpReachEntry {
                    prefix,
                    metric,
                    up_down,
                    sub_tlv: false,
                    prefix_len: 32,
                });
                off += 8;
            }
            IsisTlv::IpInternalReachability(entries)
        }
        tlv_types::IP_EXTERNAL_REACHABILITY => {
            let mut entries = Vec::new();
            let mut off = 0;
            while off + 8 <= value.len() {
                let metric_byte = value[off];
                let metric = (metric_byte & 0x7F) as u32;
                let up_down = metric_byte & 0x80 != 0;
                let ip_bytes = &value[off + 4..off + 8];
                let prefix = format!(
                    "{}.{}.{}.{}/32",
                    ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
                );
                entries.push(IpReachEntry {
                    prefix,
                    metric,
                    up_down,
                    sub_tlv: false,
                    prefix_len: 32,
                });
                off += 8;
            }
            IsisTlv::IpExternalReachability(entries)
        }
        tlv_types::PROTOCOLS_SUPPORTED => IsisTlv::ProtocolsSupported(value.to_vec()),
        tlv_types::IPV6_REACHABILITY => {
            let mut entries = Vec::new();
            let mut off = 0;
            while off < value.len() {
                if off + 4 > value.len() {
                    break;
                }
                let prefix_len = value[off];
                let metric =
                    u32::from_be_bytes([0, value[off + 1], value[off + 2], value[off + 3]]);
                off += 4;
                let prefix_byte_len = (prefix_len as usize).div_ceil(8);
                if off + prefix_byte_len > value.len() {
                    break;
                }
                let mut ip6_bytes = [0u8; 16];
                ip6_bytes[..prefix_byte_len].copy_from_slice(&value[off..off + prefix_byte_len]);
                off += prefix_byte_len;
                let prefix = format!(
                    "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}/{}",
                    u16::from_be_bytes([ip6_bytes[0], ip6_bytes[1]]),
                    u16::from_be_bytes([ip6_bytes[2], ip6_bytes[3]]),
                    u16::from_be_bytes([ip6_bytes[4], ip6_bytes[5]]),
                    u16::from_be_bytes([ip6_bytes[6], ip6_bytes[7]]),
                    u16::from_be_bytes([ip6_bytes[8], ip6_bytes[9]]),
                    u16::from_be_bytes([ip6_bytes[10], ip6_bytes[11]]),
                    u16::from_be_bytes([ip6_bytes[12], ip6_bytes[13]]),
                    u16::from_be_bytes([ip6_bytes[14], ip6_bytes[15]]),
                    prefix_len
                );
                entries.push(Ipv6ReachEntry {
                    prefix,
                    metric,
                    up_down: false,
                    prefix_len,
                });
            }
            IsisTlv::Ipv6Reachability(entries)
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
                v.push(n.pseudonode_id);
                let metric_bytes = n.metric.to_be_bytes();
                v.extend_from_slice(&metric_bytes[1..]); // 3 bytes
                v.push(0); // sub-TLV length = 0 (no sub-TLVs stored)
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
        IsisTlv::IpInternalReachability(entries) => {
            let mut v = Vec::new();
            for e in entries {
                let metric_byte = (e.metric as u8 & 0x7F) | if e.up_down { 0x80 } else { 0 };
                v.push(metric_byte);
                v.push(0); // delay metric
                v.push(0); // expense metric
                v.push(0); // error metric
                let octets: Vec<u8> = e
                    .prefix
                    .trim_end_matches("/32")
                    .split('.')
                    .map(|o| o.parse::<u8>().unwrap_or(0))
                    .collect();
                if octets.len() == 4 {
                    v.extend_from_slice(&octets);
                } else {
                    v.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
            (tlv_types::IP_INTERNAL_REACHABILITY, v)
        }
        IsisTlv::IpExternalReachability(entries) => {
            let mut v = Vec::new();
            for e in entries {
                let metric_byte = (e.metric as u8 & 0x7F) | if e.up_down { 0x80 } else { 0 };
                v.push(metric_byte);
                v.push(0); // delay metric
                v.push(0); // expense metric
                v.push(0); // error metric
                let octets: Vec<u8> = e
                    .prefix
                    .trim_end_matches("/32")
                    .split('.')
                    .map(|o| o.parse::<u8>().unwrap_or(0))
                    .collect();
                if octets.len() == 4 {
                    v.extend_from_slice(&octets);
                } else {
                    v.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
            (tlv_types::IP_EXTERNAL_REACHABILITY, v)
        }
        IsisTlv::ProtocolsSupported(nlpids) => (tlv_types::PROTOCOLS_SUPPORTED, nlpids.clone()),
        IsisTlv::Ipv6Reachability(entries) => {
            let mut v = Vec::new();
            for e in entries {
                v.push(e.prefix_len);
                let metric_bytes = e.metric.to_be_bytes();
                v.extend_from_slice(&metric_bytes[1..]); // 3 bytes wide metric
                let prefix_byte_len = (e.prefix_len as usize).div_ceil(8);
                let octets: Vec<u8> = e
                    .prefix
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .split(':')
                    .flat_map(|h| {
                        let val = u16::from_str_radix(h, 16).unwrap_or(0);
                        val.to_be_bytes().to_vec()
                    })
                    .collect();
                v.extend_from_slice(&octets[..prefix_byte_len.min(octets.len())]);
            }
            (tlv_types::IPV6_REACHABILITY, v)
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
