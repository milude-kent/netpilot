/// OSPFv2 packet codec (RFC 2328).
///
/// Supports parsing and encoding of OSPF packet headers and the
/// most common packet types: Hello, Database Description,
/// Link State Request, Link State Update, Link State Ack.
///
/// OSPF runs directly over IP (protocol 89), not over TCP/UDP.
/// On Linux we use a raw socket bound to proto 89; on macOS
/// the module provides encode/decode but no I/O.

// ── Packet types ───────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OspfPacket {
    Hello(HelloPacket),
    DbDesc(DbDescPacket),
    LsRequest(LsRequestPacket),
    LsUpdate(LsUpdatePacket),
    LsAck(LsAckPacket),
}

/// Common OSPF packet header (24 bytes, RFC 2328 §A.3.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OspfHeader {
    pub version: u8,     // always 2
    pub packet_type: u8, // 1=Hello, 2=DBDesc, 3=LSReq, 4=LSUpd, 5=LSAck
    pub packet_length: u16,
    pub router_id: u32,     // in host byte order
    pub area_id: u32,       // in host byte order
    pub checksum: u16,      // Fletcher checksum over entire packet
    pub auth_type: u16,     // 0=none, 1=simple, 2=cryptographic
    pub auth_data: [u8; 8], // authentication data
}

/// OSPF Hello packet (RFC 2328 §A.3.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelloPacket {
    pub header: OspfHeader,
    pub network_mask: u32,
    pub hello_interval_secs: u16,
    pub options: u8,
    pub router_priority: u8,
    pub dead_interval_secs: u32,
    pub designated_router: u32,
    pub backup_designated_router: u32,
    pub neighbors: Vec<u32>, // router IDs of neighbors seen
}

/// Database Description packet (RFC 2328 §A.3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbDescPacket {
    pub header: OspfHeader,
    pub interface_mtu: u16,
    pub options: u8,
    pub flags: u8, // I, M, MS bits
    pub dd_sequence_number: u32,
    pub lsa_headers: Vec<LsaHeader>,
}

/// Link State Request packet (RFC 2328 §A.3.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LsRequestPacket {
    pub header: OspfHeader,
    pub requests: Vec<LsRequestEntry>,
}

/// Link State Update packet (RFC 2328 §A.3.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LsUpdatePacket {
    pub header: OspfHeader,
    pub lsas: Vec<Lsa>,
}

/// Link State Acknowledgment packet (RFC 2328 §A.3.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LsAckPacket {
    pub header: OspfHeader,
    pub lsa_headers: Vec<LsaHeader>,
}

// ── LSA types ──────────────────────────────────────────────────

/// Common LSA header (20 bytes, RFC 2328 §A.4.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LsaHeader {
    pub ls_age: u16,
    pub ls_type: u8, // 1=Router, 2=Network, 3=Summary, 4=ASBR-Summary, 5=AS-External
    pub link_state_id: u32,
    pub advertising_router: u32,
    pub ls_sequence_number: u32,
    pub ls_checksum: u16,
    pub length: u16,
}

/// Full LSA with header and body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lsa {
    pub header: LsaHeader,
    pub body: Vec<u8>, // raw LSA body (type-specific)
}

/// A single LS Request entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LsRequestEntry {
    pub ls_type: u8,
    pub link_state_id: u32,
    pub advertising_router: u32,
}

// ── Encode / Decode ────────────────────────────────────────────

/// Errors that can occur during OSPF packet codec operations.
#[derive(Debug, thiserror::Error)]
pub enum OspfCodecError {
    #[error("packet too short: need {need} bytes, got {got}")]
    Truncated { need: usize, got: usize },
    #[error("invalid OSPF version: {0}")]
    BadVersion(u8),
    #[error("unknown packet type: {0}")]
    UnknownPacketType(u8),
    #[error("checksum verification not implemented (pass as-is)")]
    ChecksumUnimplemented,
}

/// Decode an OSPF packet from wire bytes.
pub fn decode_ospf_packet(data: &[u8]) -> Result<OspfPacket, OspfCodecError> {
    let header = decode_header(data)?;
    match header.packet_type {
        1 => decode_hello(data, header).map(OspfPacket::Hello),
        2 => decode_db_desc(data, header).map(OspfPacket::DbDesc),
        3 => decode_ls_request(data, header).map(OspfPacket::LsRequest),
        4 => decode_ls_update(data, header).map(OspfPacket::LsUpdate),
        5 => decode_ls_ack(data, header).map(OspfPacket::LsAck),
        t => Err(OspfCodecError::UnknownPacketType(t)),
    }
}

fn need(data: &[u8], n: usize) -> Result<(), OspfCodecError> {
    if data.len() < n {
        Err(OspfCodecError::Truncated {
            need: n,
            got: data.len(),
        })
    } else {
        Ok(())
    }
}

fn decode_header(data: &[u8]) -> Result<OspfHeader, OspfCodecError> {
    need(data, 24)?;
    let version = data[0];
    if version != 2 {
        return Err(OspfCodecError::BadVersion(version));
    }
    let mut auth_data = [0u8; 8];
    auth_data.copy_from_slice(&data[16..24]);
    Ok(OspfHeader {
        version,
        packet_type: data[1],
        packet_length: u16::from_be_bytes([data[2], data[3]]),
        router_id: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
        area_id: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
        checksum: u16::from_be_bytes([data[12], data[13]]),
        auth_type: u16::from_be_bytes([data[14], data[15]]),
        auth_data,
    })
}

fn decode_hello(data: &[u8], header: OspfHeader) -> Result<HelloPacket, OspfCodecError> {
    // Hello packet: header(24) + mask(4) + interval(2) + options(1) +
    //               priority(1) + dead(4) + DR(4) + BDR(4) + neighbors...
    need(data, 40)?;
    let network_mask = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);
    let hello_interval_secs = u16::from_be_bytes([data[28], data[29]]);
    let options = data[30];
    let router_priority = data[31];
    let dead_interval_secs = u32::from_be_bytes([data[32], data[33], data[34], data[35]]);
    let designated_router = u32::from_be_bytes([data[36], data[37], data[38], data[39]]);
    let backup_designated_router = u32::from_be_bytes([data[40], data[41], data[42], data[43]]);

    let mut neighbors = Vec::new();
    let mut pos = 44;
    while pos + 4 <= data.len() {
        neighbors.push(u32::from_be_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]));
        pos += 4;
    }

    Ok(HelloPacket {
        header,
        network_mask,
        hello_interval_secs,
        options,
        router_priority,
        dead_interval_secs,
        designated_router,
        backup_designated_router,
        neighbors,
    })
}

fn decode_lsa_header(data: &[u8], offset: usize) -> Result<LsaHeader, OspfCodecError> {
    need(data, offset + 20)?;
    let d = &data[offset..];
    Ok(LsaHeader {
        ls_age: u16::from_be_bytes([d[0], d[1]]),
        ls_type: d[2],
        link_state_id: u32::from_be_bytes([d[3], d[4], d[5], d[6]]),
        advertising_router: u32::from_be_bytes([d[7], d[8], d[9], d[10]]),
        ls_sequence_number: u32::from_be_bytes([d[11], d[12], d[13], d[14]]),
        ls_checksum: u16::from_be_bytes([d[15], d[16]]),
        length: u16::from_be_bytes([d[17], d[18]]),
    })
}

fn decode_db_desc(data: &[u8], header: OspfHeader) -> Result<DbDescPacket, OspfCodecError> {
    need(data, 32)?;
    let interface_mtu = u16::from_be_bytes([data[24], data[25]]);
    let options = data[26];
    let flags = data[27];
    let dd_sequence_number = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);

    let mut lsa_headers = Vec::new();
    let mut pos = 32;
    while pos + 20 <= data.len() {
        lsa_headers.push(decode_lsa_header(data, pos)?);
        pos += 20;
    }

    Ok(DbDescPacket {
        header,
        interface_mtu,
        options,
        flags,
        dd_sequence_number,
        lsa_headers,
    })
}

fn decode_ls_request(data: &[u8], header: OspfHeader) -> Result<LsRequestPacket, OspfCodecError> {
    need(data, 24)?;
    let mut requests = Vec::new();
    let mut pos = 24;
    while pos + 12 <= data.len() {
        requests.push(LsRequestEntry {
            ls_type: data[pos],
            // padding byte at pos+1
            link_state_id: u32::from_be_bytes([
                data[pos + 2],
                data[pos + 3],
                data[pos + 4],
                data[pos + 5],
            ]),
            advertising_router: u32::from_be_bytes([
                data[pos + 6],
                data[pos + 7],
                data[pos + 8],
                data[pos + 9],
            ]),
        });
        pos += 12;
    }
    Ok(LsRequestPacket { header, requests })
}

fn decode_lsa(data: &[u8], offset: usize) -> Result<Lsa, OspfCodecError> {
    let hdr = decode_lsa_header(data, offset)?;
    let lsa_len = hdr.length as usize;
    let body_start = offset + 20;
    let body_end = offset + lsa_len;
    if body_end > data.len() {
        return Err(OspfCodecError::Truncated {
            need: body_end,
            got: data.len(),
        });
    }
    Ok(Lsa {
        header: hdr,
        body: data[body_start..body_end].to_vec(),
    })
}

fn decode_ls_update(data: &[u8], header: OspfHeader) -> Result<LsUpdatePacket, OspfCodecError> {
    need(data, 28)?;
    let num_lsas = u32::from_be_bytes([data[24], data[25], data[26], data[27]]) as usize;
    let mut lsas = Vec::new();
    let mut pos = 28;
    for _ in 0..num_lsas {
        if pos + 20 > data.len() {
            break;
        }
        let lsa = decode_lsa(data, pos)?;
        pos += lsa.header.length as usize;
        lsas.push(lsa);
    }
    Ok(LsUpdatePacket { header, lsas })
}

fn decode_ls_ack(data: &[u8], header: OspfHeader) -> Result<LsAckPacket, OspfCodecError> {
    let mut lsa_headers = Vec::new();
    let mut pos = 24;
    while pos + 20 <= data.len() {
        lsa_headers.push(decode_lsa_header(data, pos)?);
        pos += 20;
    }
    Ok(LsAckPacket {
        header,
        lsa_headers,
    })
}

// ── Encode helpers ─────────────────────────────────────────────

/// Encode an OSPF header into bytes (caller must fill checksum after).
pub fn encode_header(hdr: &OspfHeader) -> Vec<u8> {
    let mut buf = Vec::with_capacity(24);
    buf.push(hdr.version);
    buf.push(hdr.packet_type);
    buf.extend_from_slice(&hdr.packet_length.to_be_bytes());
    buf.extend_from_slice(&hdr.router_id.to_be_bytes());
    buf.extend_from_slice(&hdr.area_id.to_be_bytes());
    buf.extend_from_slice(&hdr.checksum.to_be_bytes());
    buf.extend_from_slice(&hdr.auth_type.to_be_bytes());
    buf.extend_from_slice(&hdr.auth_data);
    buf
}

/// Encode a Hello packet into wire bytes.
pub fn encode_hello(pkt: &HelloPacket) -> Vec<u8> {
    let mut buf = encode_header(&pkt.header);
    buf.extend_from_slice(&pkt.network_mask.to_be_bytes());
    buf.extend_from_slice(&pkt.hello_interval_secs.to_be_bytes());
    buf.push(pkt.options);
    buf.push(pkt.router_priority);
    buf.extend_from_slice(&pkt.dead_interval_secs.to_be_bytes());
    buf.extend_from_slice(&pkt.designated_router.to_be_bytes());
    buf.extend_from_slice(&pkt.backup_designated_router.to_be_bytes());
    for &n in &pkt.neighbors {
        buf.extend_from_slice(&n.to_be_bytes());
    }
    // Update packet_length
    let len = buf.len() as u16;
    buf[2..4].copy_from_slice(&len.to_be_bytes());
    buf
}

/// Encode a Database Description packet into wire bytes.
pub fn encode_db_desc(pkt: &DbDescPacket) -> Vec<u8> {
    let mut buf = encode_header(&pkt.header);
    buf.extend_from_slice(&pkt.interface_mtu.to_be_bytes());
    buf.push(pkt.options);
    buf.push(pkt.flags);
    buf.extend_from_slice(&pkt.dd_sequence_number.to_be_bytes());
    for lsa_hdr in &pkt.lsa_headers {
        buf.extend_from_slice(&lsa_hdr.ls_age.to_be_bytes());
        buf.push(lsa_hdr.ls_type);
        buf.extend_from_slice(&lsa_hdr.link_state_id.to_be_bytes());
        buf.extend_from_slice(&lsa_hdr.advertising_router.to_be_bytes());
        buf.extend_from_slice(&lsa_hdr.ls_sequence_number.to_be_bytes());
        buf.extend_from_slice(&lsa_hdr.ls_checksum.to_be_bytes());
        buf.extend_from_slice(&lsa_hdr.length.to_be_bytes());
    }
    // Update packet_length
    let len = buf.len() as u16;
    buf[2..4].copy_from_slice(&len.to_be_bytes());
    buf
}

/// Compute the OSPF Fletcher checksum over the packet bytes.
/// The checksum and authentication fields (bytes 12-23) are
/// treated as zero during computation.
pub fn fletcher_checksum(packet: &[u8]) -> u16 {
    let mut sum0: u8 = 0;
    let mut sum1: u8 = 0;
    for (i, &byte) in packet.iter().enumerate() {
        // Skip checksum (12-13) and auth (14-23)
        if (12..24).contains(&i) {
            continue;
        }
        sum0 = sum0.wrapping_add(byte);
        sum1 = sum1.wrapping_add(sum0);
    }
    u16::from_be_bytes([sum1, sum0])
}

// ── Utility ────────────────────────────────────────────────────

/// Format a 32-bit OSPF router/area ID as dotted decimal (e.g. "1.2.3.4").
pub fn format_ospf_id(id: u32) -> String {
    let bytes = id.to_be_bytes();
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

/// Parse a dotted-decimal OSPF ID into a 32-bit value.
pub fn parse_ospf_id(s: &str) -> Option<u32> {
    let parts: Vec<u8> = s.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() == 4 {
        Some(u32::from_be_bytes([parts[0], parts[1], parts[2], parts[3]]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_router_id() -> u32 {
        u32::from_be_bytes([1, 1, 1, 1])
    }

    fn test_area_id() -> u32 {
        u32::from_be_bytes([0, 0, 0, 0])
    }

    fn make_header(pkt_type: u8, router_id: u32, area_id: u32) -> OspfHeader {
        OspfHeader {
            version: 2,
            packet_type: pkt_type,
            packet_length: 0, // filled by encode
            router_id,
            area_id,
            checksum: 0,
            auth_type: 0,
            auth_data: [0u8; 8],
        }
    }

    #[test]
    fn encode_decode_hello_empty_neighbors() {
        let hdr = make_header(1, test_router_id(), test_area_id());
        let hello = HelloPacket {
            header: hdr,
            network_mask: 0xFFFF_FF00u32
                .to_be_bytes()
                .iter()
                .fold(0u32, |a, &b| a * 256 + b as u32),
            hello_interval_secs: 10,
            options: 0x02,
            router_priority: 1,
            dead_interval_secs: 40,
            designated_router: 0,
            backup_designated_router: 0,
            neighbors: vec![],
        };
        let encoded = encode_hello(&hello);
        let decoded = decode_ospf_packet(&encoded).unwrap();
        match decoded {
            OspfPacket::Hello(h) => {
                assert_eq!(h.hello_interval_secs, 10);
                assert_eq!(h.dead_interval_secs, 40);
                assert_eq!(h.router_priority, 1);
                assert!(h.neighbors.is_empty());
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn encode_decode_hello_with_neighbors() {
        let hdr = make_header(1, test_router_id(), test_area_id());
        let neighbor1 = u32::from_be_bytes([2, 2, 2, 2]);
        let neighbor2 = u32::from_be_bytes([3, 3, 3, 3]);
        let hello = HelloPacket {
            header: hdr,
            network_mask: 0xFFFFFF00,
            hello_interval_secs: 10,
            options: 0x02,
            router_priority: 1,
            dead_interval_secs: 40,
            designated_router: test_router_id(),
            backup_designated_router: 0,
            neighbors: vec![neighbor1, neighbor2],
        };
        let encoded = encode_hello(&hello);
        let decoded = decode_ospf_packet(&encoded).unwrap();
        match decoded {
            OspfPacket::Hello(h) => {
                assert_eq!(h.neighbors.len(), 2);
                assert_eq!(h.neighbors[0], neighbor1);
                assert_eq!(h.neighbors[1], neighbor2);
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn decode_hello_truncated() {
        let data = [0u8; 30]; // less than 40 bytes needed
        let result = decode_ospf_packet(&data);
        assert!(result.is_err());
    }

    #[test]
    fn decode_bad_version() {
        let mut data = vec![0u8; 44];
        data[0] = 3; // wrong version
        let result = decode_ospf_packet(&data);
        assert!(matches!(result, Err(OspfCodecError::BadVersion(3))));
    }

    #[test]
    fn format_parse_ospf_id() {
        let id = parse_ospf_id("1.2.3.4").unwrap();
        assert_eq!(id, u32::from_be_bytes([1, 2, 3, 4]));
        assert_eq!(format_ospf_id(id), "1.2.3.4");
    }

    #[test]
    fn fletcher_checksum_known() {
        // Simple smoke test: checksum of all-zeros except header fields
        let mut pkt = vec![0u8; 44];
        pkt[0] = 2; // version
        pkt[1] = 1; // Hello
        let cs = fletcher_checksum(&pkt);
        // Result should be deterministic
        assert_ne!(cs, 0);
    }
}
