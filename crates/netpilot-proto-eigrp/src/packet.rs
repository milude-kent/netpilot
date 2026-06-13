use crate::tlv::EigrpTlv;

/// EIGRP packet header (20 bytes fixed + TLVs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EigrpHeader {
    pub version: u8, // 2
    pub opcode: EigrpOpcode,
    pub checksum: u16,
    pub flags: u32,
    pub sequence_number: u32,
    pub ack_sequence_number: u32,
    pub autonomous_system: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EigrpOpcode {
    Hello = 5,
    Update = 1,
    Query = 3,
    Reply = 4,
    Ack = 8,
}

impl EigrpOpcode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            5 => Some(Self::Hello),
            1 => Some(Self::Update),
            3 => Some(Self::Query),
            4 => Some(Self::Reply),
            8 => Some(Self::Ack),
            _ => None,
        }
    }
    pub fn to_u8(&self) -> u8 {
        match self {
            Self::Hello => 5,
            Self::Update => 1,
            Self::Query => 3,
            Self::Reply => 4,
            Self::Ack => 8,
        }
    }
}

/// Flags bitmask
pub struct EigrpFlags;
impl EigrpFlags {
    pub const INIT: u32 = 0x0001;
    pub const CONDITIONAL_RECEIVE: u32 = 0x0002;
    pub const RESTART: u32 = 0x0004;
    pub const END_OF_TABLE: u32 = 0x0008;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EigrpPacket {
    pub header: EigrpHeader,
    pub tlvs: Vec<EigrpTlv>,
}
