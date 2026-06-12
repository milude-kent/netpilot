use crate::types::FilterType;
use std::net::Ipv4Addr;

#[derive(Clone, Debug, PartialEq)]
pub enum FilterValue {
    Bool(bool),
    Int(u32),
    Pair(u16, u16),
    Quad(u8, u8, u8, u8),
    String(String),
    Bytestring(Vec<u8>),
    Ip(std::net::IpAddr),
    Mac([u8; 6]),
    Prefix(PrefixData),
    Rd(RouteDistinguisher),
    Ec(EcValue),
    Lc(LcValue),
    Bgppath(AsPath),
    Bgpmask(AsPathMask),
    Clist(Vec<ClistEntry>),
    Eclist(Vec<EcValue>),
    Lclist(Vec<LcValue>),
    IntSet(Vec<IntSetRange>),
    PrefixSet(Vec<PrefixSetEntry>),
    PairSet(Vec<PairSetRange>),
    EcSet(Vec<EcValue>),
    LcSet(Vec<LcValue>),
    Enum {
        type_name: String,
        variant: String,
    },
}

impl FilterValue {
    pub fn type_of(&self) -> FilterType {
        match self {
            FilterValue::Bool(_) => FilterType::Bool,
            FilterValue::Int(_) => FilterType::Int,
            FilterValue::Pair(_, _) => FilterType::Pair,
            FilterValue::Quad(_, _, _, _) => FilterType::Quad,
            FilterValue::String(_) => FilterType::String,
            FilterValue::Bytestring(_) => FilterType::Bytestring,
            FilterValue::Ip(_) => FilterType::Ip,
            FilterValue::Mac(_) => FilterType::Mac,
            FilterValue::Prefix(_) => FilterType::Prefix,
            FilterValue::Rd(_) => FilterType::Rd,
            FilterValue::Ec(_) => FilterType::Ec,
            FilterValue::Lc(_) => FilterType::Lc,
            FilterValue::Bgppath(_) => FilterType::Bgppath,
            FilterValue::Bgpmask(_) => FilterType::Bgpmask,
            FilterValue::Clist(_) => FilterType::Clist,
            FilterValue::Eclist(_) => FilterType::Eclist,
            FilterValue::Lclist(_) => FilterType::Lclist,
            FilterValue::IntSet(_) => FilterType::IntSet,
            FilterValue::PrefixSet(_) => FilterType::PrefixSet,
            FilterValue::PairSet(_) => FilterType::PairSet,
            FilterValue::EcSet(_) => FilterType::EcSet,
            FilterValue::LcSet(_) => FilterType::LcSet,
            FilterValue::Enum { type_name, .. } => {
                FilterType::Enum(crate::types::EnumType {
                    name: type_name.clone(),
                    values: vec![],
                })
            }
        }
    }
}

// --- Sub-types ---

#[derive(Clone, Debug, PartialEq)]
pub struct PrefixData {
    pub nettype: crate::nettype::Nettype,
    pub ip: std::net::IpAddr,
    pub length: u8,
    pub source_ip: Option<std::net::IpAddr>,
    pub source_length: Option<u8>,
    pub rd: Option<RouteDistinguisher>,
    pub maxlen: Option<u8>,
    pub asn: Option<u32>,
    pub mac: Option<[u8; 6]>,
    pub vlan_id: Option<u16>,
    pub evpn_type: Option<u8>,
    pub evpn_tag: Option<u32>,
    pub evpn_esi: Option<[u8; 10]>,
    pub router_ip: Option<std::net::IpAddr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RouteDistinguisher {
    Type0 { admin: u16, assigned: u32 },
    Type1 { ip: Ipv4Addr, assigned: u16 },
    Type2 { asn: u32, assigned: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EcValue {
    pub kind: u8,
    pub key: u16,
    pub value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LcValue {
    pub asn: u32,
    pub data1: u32,
    pub data2: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsPath {
    pub segments: Vec<AsPathSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsPathSegment {
    AsSequence(Vec<u32>),
    AsSet(Vec<u32>),
    ConfedSequence(Vec<u32>),
    ConfedSet(Vec<u32>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsPathMask {
    pub patterns: Vec<AsMaskPattern>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsMaskPattern {
    Any,
    AnyOptional,
    OneOrMore,
    Exact(u32),
    Set(Vec<u32>),
    Range(u32, u32),
}

pub type ClistEntry = (u16, u16);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntSetRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrefixSetEntry {
    pub prefix: PrefixData,
    pub is_range: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairSetRange {
    pub start: (u16, u16),
    pub end: (u16, u16),
}
