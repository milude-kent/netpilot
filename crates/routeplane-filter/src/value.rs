use crate::types::FilterType;
use std::fmt;
use std::net::Ipv4Addr;

#[derive(Clone, Debug, PartialEq, Eq)]
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
    Enum { type_name: String, variant: String },
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
            FilterValue::Enum { type_name, .. } => FilterType::Enum(crate::types::EnumType {
                name: type_name.clone(),
                values: vec![],
            }),
        }
    }
}

impl fmt::Display for FilterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterValue::Bool(v) => write!(f, "{}", if *v { "true" } else { "false" }),
            FilterValue::Int(v) => write!(f, "{v}"),
            FilterValue::Pair(a, b) => write!(f, "({a},{b})"),
            FilterValue::Quad(a, b, c, d) => write!(f, "{a}.{b}.{c}.{d}"),
            FilterValue::String(v) => write!(f, "{v}"),
            FilterValue::Bytestring(v) => {
                for byte in v {
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
            FilterValue::Ip(v) => write!(f, "{v}"),
            FilterValue::Mac(v) => {
                write!(
                    f,
                    "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    v[0], v[1], v[2], v[3], v[4], v[5]
                )
            }
            FilterValue::Prefix(v) => write!(f, "{v}"),
            FilterValue::Rd(v) => write!(f, "{v}"),
            FilterValue::Ec(v) => write!(f, "{v}"),
            FilterValue::Lc(v) => write!(f, "{v}"),
            FilterValue::Bgppath(v) => write!(f, "{v}"),
            FilterValue::Bgpmask(v) => write!(f, "{v}"),
            FilterValue::Clist(v) => {
                write!(f, "[")?;
                for (i, (a, b)) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "({a},{b})")?;
                }
                write!(f, "]")
            }
            FilterValue::Eclist(v) => {
                write!(f, "[")?;
                for (i, ec) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{ec}")?;
                }
                write!(f, "]")
            }
            FilterValue::Lclist(v) => {
                write!(f, "[")?;
                for (i, lc) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{lc}")?;
                }
                write!(f, "]")
            }
            FilterValue::IntSet(v) => {
                write!(f, "[")?;
                for (i, r) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{r}")?;
                }
                write!(f, "]")
            }
            FilterValue::PrefixSet(v) => {
                write!(f, "[")?;
                for (i, entry) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", entry.prefix)?;
                    if entry.is_range {
                        write!(f, "..")?;
                    }
                }
                write!(f, "]")
            }
            FilterValue::PairSet(v) => {
                write!(f, "[")?;
                for (i, r) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{r}")?;
                }
                write!(f, "]")
            }
            FilterValue::EcSet(v) => {
                write!(f, "[")?;
                for (i, ec) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{ec}")?;
                }
                write!(f, "]")
            }
            FilterValue::LcSet(v) => {
                write!(f, "[")?;
                for (i, lc) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{lc}")?;
                }
                write!(f, "]")
            }
            FilterValue::Enum {
                type_name: _,
                variant,
            } => write!(f, "{variant}"),
        }
    }
}

impl fmt::Display for PrefixData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.ip, self.length)
    }
}

impl fmt::Display for RouteDistinguisher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteDistinguisher::Type0 { admin, assigned } => write!(f, "{admin}:{assigned}"),
            RouteDistinguisher::Type1 { ip, assigned } => write!(f, "{ip}:{assigned}"),
            RouteDistinguisher::Type2 { asn, assigned } => write!(f, "{asn}:{assigned}"),
        }
    }
}

impl fmt::Display for EcValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{},{})", self.kind, self.key, self.value)
    }
}

impl fmt::Display for LcValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{},{})", self.asn, self.data1, self.data2)
    }
}

impl fmt::Display for AsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, seg) in self.segments.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{seg}")?;
        }
        Ok(())
    }
}

impl fmt::Display for AsPathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsPathSegment::AsSequence(asns) => {
                for (i, asn) in asns.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{asn}")?;
                }
            }
            AsPathSegment::AsSet(asns) => {
                write!(f, "[")?;
                for (i, asn) in asns.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{asn}")?;
                }
                write!(f, "]")?;
            }
            AsPathSegment::ConfedSequence(asns) => {
                write!(f, "(")?;
                for (i, asn) in asns.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{asn}")?;
                }
                write!(f, ")")?;
            }
            AsPathSegment::ConfedSet(asns) => {
                write!(f, "([")?;
                for (i, asn) in asns.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{asn}")?;
                }
                write!(f, "])")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for AsPathMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, pat) in self.patterns.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{pat}")?;
        }
        Ok(())
    }
}

impl fmt::Display for AsMaskPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsMaskPattern::Any => write!(f, "*"),
            AsMaskPattern::AnyOptional => write!(f, "?"),
            AsMaskPattern::OneOrMore => write!(f, "+"),
            AsMaskPattern::Exact(n) => write!(f, "{n}"),
            AsMaskPattern::Set(set) => {
                write!(f, "[")?;
                for (i, asn) in set.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{asn}")?;
                }
                write!(f, "]")
            }
            AsMaskPattern::Range(lo, hi) => write!(f, "{lo}..{hi}"),
        }
    }
}

impl fmt::Display for IntSetRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}..{}", self.start, self.end)
        }
    }
}

impl fmt::Display for PairSetRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.start.0, self.start.1)?;
        if self.start != self.end {
            write!(f, "..({},{})", self.end.0, self.end.1)?;
        }
        Ok(())
    }
}

// --- Sub-types ---

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
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

impl AsPath {
    pub fn first(&self) -> Option<u32> {
        self.segments.first().and_then(|seg| seg.first_asn())
    }

    pub fn last(&self) -> Option<u32> {
        self.segments.last().and_then(|seg| seg.last_asn())
    }

    pub fn last_nonaggregated(&self) -> Option<u32> {
        self.segments.iter().rev().find_map(|seg| match seg {
            AsPathSegment::AsSequence(asns) => asns.last().copied(),
            AsPathSegment::ConfedSequence(asns) => asns.last().copied(),
            _ => None,
        })
    }

    pub fn len(&self) -> usize {
        self.segments.iter().map(|seg| seg.len()).sum()
    }

    pub fn empty(&self) -> bool {
        self.len() == 0
    }

    pub fn prepend(&mut self, asn: u32) {
        match self.segments.first_mut() {
            Some(AsPathSegment::AsSequence(asns)) => {
                asns.insert(0, asn);
            }
            _ => {
                self.segments
                    .insert(0, AsPathSegment::AsSequence(vec![asn]));
            }
        }
    }

    pub fn delete(&mut self, asn: u32) {
        for seg in &mut self.segments {
            match seg {
                AsPathSegment::AsSequence(asns)
                | AsPathSegment::AsSet(asns)
                | AsPathSegment::ConfedSequence(asns)
                | AsPathSegment::ConfedSet(asns) => {
                    asns.retain(|a| *a != asn);
                }
            }
        }
        self.segments.retain(|seg| seg.len() > 0);
    }

    pub fn filter<F>(&mut self, predicate: F)
    where
        F: Fn(&u32) -> bool,
    {
        for seg in &mut self.segments {
            match seg {
                AsPathSegment::AsSequence(asns)
                | AsPathSegment::AsSet(asns)
                | AsPathSegment::ConfedSequence(asns)
                | AsPathSegment::ConfedSet(asns) => {
                    asns.retain(|a| predicate(a));
                }
            }
        }
        self.segments.retain(|seg| seg.len() > 0);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsPathSegment {
    AsSequence(Vec<u32>),
    AsSet(Vec<u32>),
    ConfedSequence(Vec<u32>),
    ConfedSet(Vec<u32>),
}

impl AsPathSegment {
    fn first_asn(&self) -> Option<u32> {
        self.asns().first().copied()
    }

    fn last_asn(&self) -> Option<u32> {
        self.asns().last().copied()
    }

    fn len(&self) -> usize {
        self.asns().len()
    }

    pub fn asns(&self) -> &[u32] {
        match self {
            AsPathSegment::AsSequence(asns) => asns,
            AsPathSegment::AsSet(asns) => asns,
            AsPathSegment::ConfedSequence(asns) => asns,
            AsPathSegment::ConfedSet(asns) => asns,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsPathMask {
    pub patterns: Vec<AsMaskPattern>,
}

impl AsPathMask {
    /// Match this mask against an AS path using recursive backtracking.
    pub fn matches(&self, path: &AsPath) -> bool {
        let flat: Vec<u32> = path
            .segments
            .iter()
            .flat_map(|seg| seg.asns().to_vec())
            .collect();
        self.match_recursive(&flat, 0, 0)
    }

    fn match_recursive(&self, asns: &[u32], pat_idx: usize, asn_idx: usize) -> bool {
        if pat_idx >= self.patterns.len() {
            return asn_idx >= asns.len();
        }

        match &self.patterns[pat_idx] {
            AsMaskPattern::Exact(n) => {
                if asn_idx < asns.len() && asns[asn_idx] == *n {
                    self.match_recursive(asns, pat_idx + 1, asn_idx + 1)
                } else {
                    false
                }
            }
            AsMaskPattern::Set(set) => {
                if asn_idx < asns.len() && set.contains(&asns[asn_idx]) {
                    self.match_recursive(asns, pat_idx + 1, asn_idx + 1)
                } else {
                    false
                }
            }
            AsMaskPattern::Range(lo, hi) => {
                if asn_idx < asns.len() && asns[asn_idx] >= *lo && asns[asn_idx] <= *hi {
                    self.match_recursive(asns, pat_idx + 1, asn_idx + 1)
                } else {
                    false
                }
            }
            AsMaskPattern::Any => {
                if asn_idx < asns.len() {
                    self.match_recursive(asns, pat_idx + 1, asn_idx + 1)
                } else {
                    false
                }
            }
            AsMaskPattern::AnyOptional => {
                // try skipping
                if self.match_recursive(asns, pat_idx + 1, asn_idx) {
                    return true;
                }
                // or consume one
                if asn_idx < asns.len() {
                    self.match_recursive(asns, pat_idx + 1, asn_idx + 1)
                } else {
                    self.match_recursive(asns, pat_idx + 1, asn_idx)
                }
            }
            AsMaskPattern::OneOrMore => {
                if asn_idx >= asns.len() {
                    return false;
                }
                // consume at least one, then greedy
                let mut idx = asn_idx + 1;
                loop {
                    if self.match_recursive(asns, pat_idx + 1, idx) {
                        return true;
                    }
                    if idx >= asns.len() {
                        break;
                    }
                    idx += 1;
                }
                false
            }
        }
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixSetEntry {
    pub prefix: PrefixData,
    pub is_range: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairSetRange {
    pub start: (u16, u16),
    pub end: (u16, u16),
}

// --- Community list operations ---

pub fn clist_add(list: &mut Vec<ClistEntry>, entry: ClistEntry) {
    if !list.contains(&entry) {
        list.push(entry);
    }
}

pub fn clist_delete(list: &mut Vec<ClistEntry>, entry: &ClistEntry) {
    list.retain(|e| e != entry);
}

pub fn clist_filter<F>(list: &mut Vec<ClistEntry>, predicate: F)
where
    F: Fn(&ClistEntry) -> bool,
{
    list.retain(|e| predicate(e));
}

pub fn clist_min(list: &[ClistEntry]) -> Option<ClistEntry> {
    list.iter().min_by_key(|(a, d)| (*a, *d)).copied()
}

pub fn clist_max(list: &[ClistEntry]) -> Option<ClistEntry> {
    list.iter().max_by_key(|(a, d)| (*a, *d)).copied()
}

pub fn eclist_add(list: &mut Vec<EcValue>, entry: EcValue) {
    if !list.contains(&entry) {
        list.push(entry);
    }
}

pub fn eclist_delete(list: &mut Vec<EcValue>, entry: &EcValue) {
    list.retain(|e| e != entry);
}

pub fn eclist_filter<F>(list: &mut Vec<EcValue>, predicate: F)
where
    F: Fn(&EcValue) -> bool,
{
    list.retain(|e| predicate(e));
}

pub fn eclist_min(list: &[EcValue]) -> Option<&EcValue> {
    list.iter().min_by_key(|ec| (ec.kind, ec.key, ec.value))
}

pub fn eclist_max(list: &[EcValue]) -> Option<&EcValue> {
    list.iter().max_by_key(|ec| (ec.kind, ec.key, ec.value))
}

pub fn lclist_add(list: &mut Vec<LcValue>, entry: LcValue) {
    if !list.contains(&entry) {
        list.push(entry);
    }
}

pub fn lclist_delete(list: &mut Vec<LcValue>, entry: &LcValue) {
    list.retain(|e| e != entry);
}

pub fn lclist_filter<F>(list: &mut Vec<LcValue>, predicate: F)
where
    F: Fn(&LcValue) -> bool,
{
    list.retain(|e| predicate(e));
}

pub fn lclist_min(list: &[LcValue]) -> Option<&LcValue> {
    list.iter().min_by_key(|lc| (lc.asn, lc.data1, lc.data2))
}

pub fn lclist_max(list: &[LcValue]) -> Option<&LcValue> {
    list.iter().max_by_key(|lc| (lc.asn, lc.data1, lc.data2))
}
