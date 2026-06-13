use crate::tlv::IsisTlv;

/// Common IS-IS header (8 bytes). All PDU types share this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsisHeader {
    pub protocol_id: u8,          // 0x83 for IS-IS
    pub header_length: u8,        // length of fixed header
    pub version: u8,              // 1
    pub system_id_length: u8,     // 0 (indicates 6-byte system IDs)
    pub pdu_type: PduType,        // encoded in the type field
    pub version2: u8,             // 1
    pub reserved: u8,
    pub max_area_addresses: u8,   // typically 3
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PduType {
    Level1LanIih = 15,
    Level2LanIih = 16,
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
        matches!(self, Self::Level1LanIih | Self::Level2LanIih)
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
    Lsp(LspPacket),
    Csnp(CsnpPacket),
    Psnp(PsnpPacket),
}

/// IS-IS Hello (IIH) packet — LAN variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IihPacket {
    pub circuit_type: u8,           // 1=L1, 2=L2, 3=L1L2
    pub source_id: String,          // 6-byte system ID
    pub holding_time_secs: u16,
    pub pdu_length: u16,
    pub priority: u8,               // DIS priority (0-127)
    pub lan_id: Option<String>,     // DIS system ID + pseudonode
    pub neighbors: Vec<String>,     // system IDs of neighbors seen
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
    pub system_id: String,       // 6-byte system ID
    pub pseudonode_id: u8,       // 0 = real node, 1-255 = pseudonode
    pub fragment: u32,           // fragment number
}

impl LspId {
    pub fn new(system_id: &str, pseudonode_id: u8, fragment: u32) -> Self {
        Self { system_id: system_id.to_string(), pseudonode_id, fragment }
    }

    pub fn display(&self) -> String {
        format!("{}.{:02x}-{:02x}", self.system_id, self.pseudonode_id, self.fragment)
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
            _ => {
                // LSP/CSNP/PSNP: append TLVs only (simplified)
                let tlvs = match &self.body {
                    IsisPacketBody::Lsp(lsp) => &lsp.tlvs,
                    IsisPacketBody::Csnp(csnp) => &csnp.tlvs,
                    IsisPacketBody::Psnp(psnp) => &psnp.tlvs,
                    _ => unreachable!(),
                };
                let tlv_bytes = crate::tlv::build_tlvs(tlvs);
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

        let pdu_type = PduType::from_u8(data[4])
            .ok_or_else(|| format!("unknown PDU type: {}", data[4]))?;

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
        let tlvs = crate::tlv::parse_tlvs(body_data);

        let body = match pdu_type {
            PduType::Level1LanIih | PduType::Level2LanIih => {
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
            _ => {
                // Simplified: LSP/CSNP/PSNP just store TLVs
                IsisPacketBody::Lsp(LspPacket {
                    pdu_length: 0,
                    remaining_lifetime_secs: 1200,
                    lsp_id: LspId::new("0000.0000.0000", 0, 0),
                    sequence_number: 0,
                    checksum: 0,
                    flags: Default::default(),
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
            if bytes.len() == 2 {
                bytes
            } else {
                vec![0, 0]
            }
        })
        .collect()
}

fn bytes_to_system_id(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(6)
        .enumerate()
        .map(|(i, b)| {
            format!(
                "{}{:02x}",
                if i % 2 == 0 && i > 0 { "." } else { "" },
                b
            )
        })
        .collect::<String>()
}
