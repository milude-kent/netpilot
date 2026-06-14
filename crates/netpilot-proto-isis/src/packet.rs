use crate::tlv::IsisTlv;

/// Common IS-IS header (8 bytes). All PDU types share this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsisHeader {
    pub protocol_id: u8,      // 0x83 for IS-IS
    pub header_length: u8,    // length of fixed header
    pub version: u8,          // 1
    pub system_id_length: u8, // 0 (indicates 6-byte system IDs)
    pub pdu_type: PduType,    // encoded in the type field
    pub version2: u8,         // 1
    pub reserved: u8,
    pub max_area_addresses: u8, // typically 3
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PduType {
    Level1LanIih = 15,
    Level2LanIih = 16,
    P2pIih = 17,
    Level1Lsp = 18,
    Level2Lsp = 20,
    Level1Csnp = 24,
    Level2Csnp = 25,
    Level1Psnp = 26,
    Level2Psnp = 27,
}

impl PduType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            15 => Some(Self::Level1LanIih),
            16 => Some(Self::Level2LanIih),
            17 => Some(Self::P2pIih),
            18 => Some(Self::Level1Lsp),
            20 => Some(Self::Level2Lsp),
            24 => Some(Self::Level1Csnp),
            25 => Some(Self::Level2Csnp),
            26 => Some(Self::Level1Psnp),
            27 => Some(Self::Level2Psnp),
            _ => None,
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            Self::Level1LanIih => 15,
            Self::Level2LanIih => 16,
            Self::P2pIih => 17,
            Self::Level1Lsp => 18,
            Self::Level2Lsp => 20,
            Self::Level1Csnp => 24,
            Self::Level2Csnp => 25,
            Self::Level1Psnp => 26,
            Self::Level2Psnp => 27,
        }
    }

    pub fn is_lsp(&self) -> bool {
        matches!(self, Self::Level1Lsp | Self::Level2Lsp)
    }

    pub fn is_hello(&self) -> bool {
        matches!(self, Self::Level1LanIih | Self::Level2LanIih | Self::P2pIih)
    }
}

/// Top-level IS-IS packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsisPacket {
    pub header: IsisHeader,
    pub body: IsisPacketBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IsisPacketBody {
    Iih(IihPacket),
    P2pIih(P2pIihPacket),
    Lsp(LspPacket),
    Csnp(CsnpPacket),
    Psnp(PsnpPacket),
}

/// IS-IS Hello (IIH) packet — LAN variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IihPacket {
    pub circuit_type: u8,  // 1=L1, 2=L2, 3=L1L2
    pub source_id: String, // 6-byte system ID
    pub holding_time_secs: u16,
    pub pdu_length: u16,
    pub priority: u8,           // DIS priority (0-127)
    pub lan_id: Option<String>, // DIS system ID + pseudonode
    pub neighbors: Vec<String>, // system IDs of neighbors seen
    pub tlvs: Vec<IsisTlv>,
}

/// IS-IS Point-to-Point IIH (PDU type 17).
/// Different from LAN IIH: no priority/lan-id, has local_circuit_id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2pIihPacket {
    pub circuit_type: u8,
    pub source_id: String, // 6-byte system ID
    pub holding_time_secs: u16,
    pub pdu_length: u16,
    pub local_circuit_id: u32, // unique per circuit
    pub tlvs: Vec<IsisTlv>,
}

/// Link State PDU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LspPacket {
    pub pdu_length: u16,
    pub remaining_lifetime_secs: u16,
    pub lsp_id: LspId,
    pub sequence_number: u32,
    pub checksum: u16,
    pub flags: LspFlags,
    pub tlvs: Vec<IsisTlv>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LspId {
    pub system_id: String, // 6-byte system ID
    pub pseudonode_id: u8, // 0 = real node, 1-255 = pseudonode
    pub fragment: u32,     // fragment number
}

impl LspId {
    pub fn new(system_id: &str, pseudonode_id: u8, fragment: u32) -> Self {
        Self {
            system_id: system_id.to_string(),
            pseudonode_id,
            fragment,
        }
    }

    pub fn display(&self) -> String {
        format!(
            "{}.{:02x}-{:02x}",
            self.system_id, self.pseudonode_id, self.fragment
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LspFlags {
    pub partition_repair: bool,
    pub attached_l2: bool,
    pub attached_l1: bool,
    pub overload: bool,
}

/// Complete Sequence Number PDU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsnpPacket {
    pub pdu_length: u16,
    pub source_id: String,
    pub start_lsp_id: Option<LspId>,
    pub end_lsp_id: Option<LspId>,
    pub lsp_entries: Vec<CsnpLspEntry>,
    pub tlvs: Vec<IsisTlv>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsnpLspEntry {
    pub lsp_id: LspId,
    pub sequence_number: u32,
    pub remaining_lifetime_secs: u16,
    pub checksum: u16,
}

/// Partial Sequence Number PDU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsnpPacket {
    pub pdu_length: u16,
    pub source_id: String,
    pub lsp_entries: Vec<CsnpLspEntry>,
    pub tlvs: Vec<IsisTlv>,
}

impl IsisPacket {
    /// Encode this packet to wire format bytes.
    ///
    /// The buffer is built incrementally with computed values (8-byte
    /// common header followed by an arbitrary TLV chain). Clippy's
    /// `vec_init_then_push` lint suggests a `vec![..]` literal, which
    /// doesn't fit the per-field push-then-extend pattern used here.
    #[allow(clippy::vec_init_then_push)]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Common header (8 bytes)
        buf.push(self.header.protocol_id);
        buf.push(self.header.header_length);
        buf.push(self.header.version);
        buf.push(self.header.system_id_length);
        buf.push(self.header.pdu_type.to_u8());
        buf.push(self.header.version2);
        buf.push(self.header.reserved);
        buf.push(self.header.max_area_addresses);

        // Encode body based on PDU type
        match &self.body {
            IsisPacketBody::Iih(iih) => {
                buf.push(iih.circuit_type);
                // Source ID: 6 bytes
                let src_bytes = system_id_to_bytes(&iih.source_id);
                buf.extend_from_slice(&src_bytes);
                buf.extend_from_slice(&iih.holding_time_secs.to_be_bytes());
                buf.extend_from_slice(&iih.pdu_length.to_be_bytes());
                buf.push(iih.priority);
                // LAN ID: 7 bytes (system_id + pseudonode)
                if let Some(ref lan) = iih.lan_id {
                    buf.extend_from_slice(&system_id_to_bytes(&lan[..13]));
                    if lan.len() > 14 {
                        buf.push(u8::from_str_radix(&lan[14..16], 16).unwrap_or(0));
                    } else {
                        buf.push(0);
                    }
                } else {
                    buf.extend_from_slice(&[0u8; 7]);
                }
                // TLV area
                let tlv_bytes = crate::tlv::build_tlvs(&iih.tlvs);
                buf.extend_from_slice(&tlv_bytes);
            }
            IsisPacketBody::P2pIih(p2p) => {
                buf.push(p2p.circuit_type);
                let src_bytes = system_id_to_bytes(&p2p.source_id);
                buf.extend_from_slice(&src_bytes);
                buf.extend_from_slice(&p2p.holding_time_secs.to_be_bytes());
                buf.extend_from_slice(&p2p.pdu_length.to_be_bytes());
                buf.push(p2p.local_circuit_id as u8);
                let tlv_bytes = crate::tlv::build_tlvs(&p2p.tlvs);
                buf.extend_from_slice(&tlv_bytes);
            }
            IsisPacketBody::Lsp(lsp) => {
                // LSP fixed header: remaining_lifetime(2) + lsp_id(8) +
                //   sequence_number(4) + checksum(2) + type_block(1) = 17 bytes
                buf.extend_from_slice(&lsp.remaining_lifetime_secs.to_be_bytes());
                buf.extend_from_slice(&lsp_id_to_bytes(&lsp.lsp_id));
                buf.extend_from_slice(&lsp.sequence_number.to_be_bytes());
                buf.extend_from_slice(&lsp.checksum.to_be_bytes());
                let mut type_block: u8 = 0;
                if lsp.flags.partition_repair {
                    type_block |= 0x80;
                }
                if lsp.flags.attached_l2 {
                    type_block |= 0x40;
                }
                if lsp.flags.attached_l1 {
                    type_block |= 0x20;
                }
                if lsp.flags.overload {
                    type_block |= 0x04;
                }
                buf.push(type_block);
                // TLV area
                let tlv_bytes = crate::tlv::build_tlvs(&lsp.tlvs);
                buf.extend_from_slice(&tlv_bytes);
            }
            IsisPacketBody::Csnp(csnp) => {
                // CSNP fixed header: source_id(6) + start_lsp_id(8) + end_lsp_id(8) = 22 bytes
                buf.extend_from_slice(&system_id_to_bytes(&csnp.source_id));
                if let Some(ref start) = csnp.start_lsp_id {
                    buf.extend_from_slice(&lsp_id_to_bytes(start));
                } else {
                    buf.extend_from_slice(&[0u8; 8]);
                }
                if let Some(ref end) = csnp.end_lsp_id {
                    buf.extend_from_slice(&lsp_id_to_bytes(end));
                } else {
                    buf.extend_from_slice(&[0u8; 8]);
                }
                // LSP entries as TLV type 9
                if !csnp.lsp_entries.is_empty() {
                    buf.push(9); // LSP Entries TLV type
                    let entry_bytes = encode_lsp_entries(&csnp.lsp_entries);
                    buf.push(entry_bytes.len() as u8);
                    buf.extend_from_slice(&entry_bytes);
                }
                // Additional TLV area
                let tlv_bytes = crate::tlv::build_tlvs(&csnp.tlvs);
                buf.extend_from_slice(&tlv_bytes);
            }
            IsisPacketBody::Psnp(psnp) => {
                // PSNP fixed header: source_id(6)
                buf.extend_from_slice(&system_id_to_bytes(&psnp.source_id));
                // LSP entries as TLV type 9
                if !psnp.lsp_entries.is_empty() {
                    buf.push(9); // LSP Entries TLV type
                    let entry_bytes = encode_lsp_entries(&psnp.lsp_entries);
                    buf.push(entry_bytes.len() as u8);
                    buf.extend_from_slice(&entry_bytes);
                }
                // Additional TLV area
                let tlv_bytes = crate::tlv::build_tlvs(&psnp.tlvs);
                buf.extend_from_slice(&tlv_bytes);
            }
        }

        buf
    }

    /// Decode wire format bytes into an IsisPacket.
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.len() < 8 {
            return Err("packet too short for ISIS header".into());
        }

        let pdu_type =
            PduType::from_u8(data[4]).ok_or_else(|| format!("unknown PDU type: {}", data[4]))?;

        let header = IsisHeader {
            protocol_id: data[0],
            header_length: data[1],
            version: data[2],
            system_id_length: data[3],
            pdu_type,
            version2: data[5],
            reserved: data[6],
            max_area_addresses: data[7],
        };

        let body_data = &data[8..];

        let body = match pdu_type {
            PduType::P2pIih => {
                // P2P IIH: circuit_type(1) + source_id(6) + holding_time(2) +
                //          pdu_length(2) + local_circuit_id(1) + TLVs
                let tlvs = crate::tlv::parse_tlvs(body_data);
                if body_data.len() < 12 {
                    return Err("P2P IIH too short".into());
                }
                IsisPacketBody::P2pIih(P2pIihPacket {
                    circuit_type: body_data[0],
                    source_id: bytes_to_system_id(&body_data[1..7]),
                    holding_time_secs: u16::from_be_bytes([body_data[7], body_data[8]]),
                    pdu_length: u16::from_be_bytes([body_data[9], body_data[10]]),
                    local_circuit_id: body_data[11] as u32,
                    tlvs,
                })
            }
            PduType::Level1LanIih | PduType::Level2LanIih => {
                // IIH: existing decode preserved as-is
                let tlvs = crate::tlv::parse_tlvs(body_data);
                if body_data.len() < 15 {
                    return Err("IIH too short".into());
                }
                IsisPacketBody::Iih(IihPacket {
                    circuit_type: body_data[0],
                    source_id: bytes_to_system_id(&body_data[1..7]),
                    holding_time_secs: u16::from_be_bytes([body_data[7], body_data[8]]),
                    pdu_length: u16::from_be_bytes([body_data[9], body_data[10]]),
                    priority: body_data[11],
                    lan_id: if body_data[12..19].iter().all(|b| *b == 0) {
                        None
                    } else {
                        Some(format!(
                            "{}.{:02x}",
                            bytes_to_system_id(&body_data[12..18]),
                            body_data[18]
                        ))
                    },
                    neighbors: vec![], // extracted from TLVs
                    tlvs,
                })
            }
            PduType::Level1Lsp | PduType::Level2Lsp => {
                // LSP fixed header after common header: 17 bytes
                //   remaining_lifetime: u16  (body_data[0..2])
                //   lsp_id:              8B  (body_data[2..10])
                //   sequence_number:    u32  (body_data[10..14])
                //   checksum:           u16  (body_data[14..16])
                //   type_block (flags):  u8  (body_data[16])
                // TLVs start at body_data[17..]
                if body_data.len() < 17 {
                    return Err("LSP too short for fixed header".into());
                }
                let remaining_lifetime_secs = u16::from_be_bytes([body_data[0], body_data[1]]);
                let lsp_id = bytes_to_lsp_id(&body_data[2..10]);
                let sequence_number = u32::from_be_bytes([
                    body_data[10],
                    body_data[11],
                    body_data[12],
                    body_data[13],
                ]);
                let checksum = u16::from_be_bytes([body_data[14], body_data[15]]);
                let type_block = body_data[16];
                let flags = LspFlags {
                    partition_repair: (type_block & 0x80) != 0,
                    attached_l2: (type_block & 0x40) != 0,
                    attached_l1: (type_block & 0x20) != 0,
                    overload: (type_block & 0x04) != 0,
                };
                let pdu_length = data.len() as u16;
                let tlvs = crate::tlv::parse_tlvs(&body_data[17..]);
                IsisPacketBody::Lsp(LspPacket {
                    pdu_length,
                    remaining_lifetime_secs,
                    lsp_id,
                    sequence_number,
                    checksum,
                    flags,
                    tlvs,
                })
            }
            PduType::Level1Csnp | PduType::Level2Csnp => {
                // CSNP fixed header after common header: 22 bytes
                //   source_id:     6B  (body_data[0..6])
                //   start_lsp_id:  8B  (body_data[6..14])
                //   end_lsp_id:    8B  (body_data[14..22])
                // TLVs start at body_data[22..]
                if body_data.len() < 22 {
                    return Err("CSNP too short for fixed header".into());
                }
                let source_id = bytes_to_system_id(&body_data[0..6]);
                let start_lsp_id = bytes_to_lsp_id(&body_data[6..14]);
                let end_lsp_id = bytes_to_lsp_id(&body_data[14..22]);
                let pdu_length = data.len() as u16;
                let tlvs = crate::tlv::parse_tlvs(&body_data[22..]);
                let lsp_entries = extract_lsp_entries(&tlvs);
                IsisPacketBody::Csnp(CsnpPacket {
                    pdu_length,
                    source_id,
                    start_lsp_id: Some(start_lsp_id),
                    end_lsp_id: Some(end_lsp_id),
                    lsp_entries,
                    tlvs,
                })
            }
            PduType::Level1Psnp | PduType::Level2Psnp => {
                // PSNP fixed header after common header: 6 bytes
                //   source_id:     6B  (body_data[0..6])
                // TLVs start at body_data[6..]
                if body_data.len() < 6 {
                    return Err("PSNP too short for fixed header".into());
                }
                let source_id = bytes_to_system_id(&body_data[0..6]);
                let pdu_length = data.len() as u16;
                let tlvs = crate::tlv::parse_tlvs(&body_data[6..]);
                let lsp_entries = extract_lsp_entries(&tlvs);
                IsisPacketBody::Psnp(PsnpPacket {
                    pdu_length,
                    source_id,
                    lsp_entries,
                    tlvs,
                })
            }
        };

        Ok(IsisPacket { header, body })
    }
}

fn system_id_to_bytes(id: &str) -> Vec<u8> {
    id.split('.')
        .flat_map(|s| {
            let bytes = hex::decode(s).unwrap_or_default();
            if bytes.len() == 2 { bytes } else { vec![0, 0] }
        })
        .collect()
}

fn bytes_to_system_id(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(6)
        .enumerate()
        .map(|(i, b)| format!("{}{:02x}", if i % 2 == 0 && i > 0 { "." } else { "" }, b))
        .collect::<String>()
}

/// Parse 8 bytes into an LspId: system_id[6] + pseudonode_id[1] + fragment[1].
fn bytes_to_lsp_id(bytes: &[u8]) -> LspId {
    LspId {
        system_id: bytes_to_system_id(&bytes[0..6]),
        pseudonode_id: bytes[6],
        fragment: bytes[7] as u32,
    }
}

/// Encode an LspId into 8 bytes: system_id[6] + pseudonode_id[1] + fragment[1].
fn lsp_id_to_bytes(lsp_id: &LspId) -> Vec<u8> {
    let mut buf = system_id_to_bytes(&lsp_id.system_id);
    buf.push(lsp_id.pseudonode_id);
    buf.push(lsp_id.fragment as u8);
    buf
}

/// IS-IS TLV type 9: LSP Entries.
const TLV_TYPE_LSP_ENTRIES: u8 = 9;

/// Extract LSP entries from TLV type 9 (LSP Entries TLV).
/// Each entry is 16 bytes: remaining_lifetime(2) + lsp_id(8) + sequence_number(4) + checksum(2).
fn extract_lsp_entries(tlvs: &[IsisTlv]) -> Vec<CsnpLspEntry> {
    let mut entries = Vec::new();
    for tlv in tlvs {
        if let IsisTlv::Unknown { type_code, value } = tlv
            && *type_code == TLV_TYPE_LSP_ENTRIES
        {
            // Each LSP entry is 16 bytes
            for chunk in value.chunks(16) {
                if chunk.len() < 16 {
                    break;
                }
                entries.push(CsnpLspEntry {
                    remaining_lifetime_secs: u16::from_be_bytes([chunk[0], chunk[1]]),
                    lsp_id: bytes_to_lsp_id(&chunk[2..10]),
                    sequence_number: u32::from_be_bytes([
                        chunk[10], chunk[11], chunk[12], chunk[13],
                    ]),
                    checksum: u16::from_be_bytes([chunk[14], chunk[15]]),
                });
            }
        }
    }
    entries
}

/// Encode LSP entries into raw bytes for TLV type 9.
/// Each entry: remaining_lifetime(2) + lsp_id(8) + sequence_number(4) + checksum(2) = 16 bytes.
fn encode_lsp_entries(entries: &[CsnpLspEntry]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(entries.len() * 16);
    for entry in entries {
        buf.extend_from_slice(&entry.remaining_lifetime_secs.to_be_bytes());
        buf.extend_from_slice(&lsp_id_to_bytes(&entry.lsp_id));
        buf.extend_from_slice(&entry.sequence_number.to_be_bytes());
        buf.extend_from_slice(&entry.checksum.to_be_bytes());
    }
    buf
}
