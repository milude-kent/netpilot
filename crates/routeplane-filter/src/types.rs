use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FilterType {
    Bool,
    Int,
    Pair,
    Quad,
    String,
    Bytestring,
    Ip,
    Mac,
    Prefix,
    Rd,
    Ec,
    Lc,
    Bgppath,
    Bgpmask,
    Clist,
    Eclist,
    Lclist,
    IntSet,
    PrefixSet,
    PairSet,
    EcSet,
    LcSet,
    Enum(EnumType),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EnumType {
    pub name: String,
    pub values: Vec<String>,
}

impl fmt::Display for FilterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterType::Bool => write!(f, "bool"),
            FilterType::Int => write!(f, "int"),
            FilterType::Pair => write!(f, "pair"),
            FilterType::Quad => write!(f, "quad"),
            FilterType::String => write!(f, "string"),
            FilterType::Bytestring => write!(f, "bytestring"),
            FilterType::Ip => write!(f, "ip"),
            FilterType::Mac => write!(f, "mac"),
            FilterType::Prefix => write!(f, "prefix"),
            FilterType::Rd => write!(f, "rd"),
            FilterType::Ec => write!(f, "ec"),
            FilterType::Lc => write!(f, "lc"),
            FilterType::Bgppath => write!(f, "bgppath"),
            FilterType::Bgpmask => write!(f, "bgpmask"),
            FilterType::Clist => write!(f, "clist"),
            FilterType::Eclist => write!(f, "eclist"),
            FilterType::Lclist => write!(f, "lclist"),
            FilterType::IntSet => write!(f, "int set"),
            FilterType::PrefixSet => write!(f, "prefix set"),
            FilterType::PairSet => write!(f, "pair set"),
            FilterType::EcSet => write!(f, "ec set"),
            FilterType::LcSet => write!(f, "lc set"),
            FilterType::Enum(et) => write!(f, "enum {}", et.name),
        }
    }
}
