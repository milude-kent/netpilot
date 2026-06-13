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

#[derive(Clone, Debug, PartialEq, Eq)]
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
