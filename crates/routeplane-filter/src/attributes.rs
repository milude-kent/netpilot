use crate::types::FilterType;
use crate::value::FilterValue;
use std::collections::HashMap;

pub trait RouteAttribute {
    fn name(&self) -> &str;
    fn attr_type(&self) -> FilterType;
    fn read(&self) -> FilterValue;
    fn write(&mut self, value: FilterValue) -> Result<(), String>;
    fn is_read_only(&self) -> bool;
}

fn default_value(attr_type: &FilterType) -> FilterValue {
    match attr_type {
        FilterType::Bool => FilterValue::Bool(false),
        FilterType::Int => FilterValue::Int(0),
        FilterType::Pair => FilterValue::Pair(0, 0),
        FilterType::Quad => FilterValue::Quad(0, 0, 0, 0),
        FilterType::String => FilterValue::String(String::new()),
        FilterType::Bytestring => FilterValue::Bytestring(Vec::new()),
        FilterType::Ip => {
            FilterValue::Ip(std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)))
        }
        FilterType::Mac => FilterValue::Mac([0; 6]),
        FilterType::Prefix => FilterValue::Prefix(crate::value::PrefixData {
            nettype: crate::nettype::Nettype::Ip4,
            ip: std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
            length: 0,
            source_ip: None,
            source_length: None,
            rd: None,
            maxlen: None,
            asn: None,
            mac: None,
            vlan_id: None,
            evpn_type: None,
            evpn_tag: None,
            evpn_esi: None,
            router_ip: None,
        }),
        FilterType::Rd => FilterValue::Rd(crate::value::RouteDistinguisher::Type0 {
            admin: 0,
            assigned: 0,
        }),
        FilterType::Ec => FilterValue::Ec(crate::value::EcValue {
            kind: 0,
            key: 0,
            value: 0,
        }),
        FilterType::Lc => FilterValue::Lc(crate::value::LcValue {
            asn: 0,
            data1: 0,
            data2: 0,
        }),
        FilterType::Bgppath => {
            FilterValue::Bgppath(crate::value::AsPath { segments: vec![] })
        }
        FilterType::Bgpmask => {
            FilterValue::Bgpmask(crate::value::AsPathMask { patterns: vec![] })
        }
        FilterType::Clist => FilterValue::Clist(Vec::new()),
        FilterType::Eclist => FilterValue::Eclist(Vec::new()),
        FilterType::Lclist => FilterValue::Lclist(Vec::new()),
        FilterType::IntSet => FilterValue::IntSet(Vec::new()),
        FilterType::PrefixSet => FilterValue::PrefixSet(Vec::new()),
        FilterType::PairSet => FilterValue::PairSet(Vec::new()),
        FilterType::EcSet => FilterValue::EcSet(Vec::new()),
        FilterType::LcSet => FilterValue::LcSet(Vec::new()),
        FilterType::Enum(_) => FilterValue::String(String::new()),
    }
}

pub struct AttributeRegistry {
    attrs: HashMap<String, Box<dyn RouteAttribute + Send + Sync>>,
}

impl AttributeRegistry {
    pub fn new() -> Self {
        AttributeRegistry {
            attrs: HashMap::new(),
        }
    }

    pub fn register(&mut self, attr: impl RouteAttribute + Send + Sync + 'static) {
        let name = attr.name().to_string();
        self.attrs.insert(name, Box::new(attr));
    }

    pub fn read(&self, name: &str) -> Result<FilterValue, String> {
        self.attrs
            .get(name)
            .map(|a| a.read())
            .ok_or_else(|| format!("attribute not defined: {name}"))
    }

    pub fn write(&mut self, name: &str, value: FilterValue) -> Result<(), String> {
        match self.attrs.get_mut(name) {
            Some(attr) => attr.write(value),
            None => Err(format!("attribute not defined: {name}")),
        }
    }

    pub fn is_defined(&self, name: &str) -> bool {
        self.attrs.contains_key(name)
    }

    pub fn unset(&mut self, name: &str) -> Result<(), String> {
        match self.attrs.get(name) {
            Some(attr) if attr.is_read_only() => {
                Err(format!("cannot unset read-only attribute: {name}"))
            }
            Some(attr) => {
                let default = default_value(&attr.attr_type());
                self.attrs
                    .get_mut(name)
                    .unwrap()
                    .write(default)
                    .map_err(|e| format!("unset failed for {name}: {e}"))
            }
            None => Err(format!("attribute not defined: {name}")),
        }
    }
}

impl Default for AttributeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct CustomIntAttribute {
    name: String,
    value: u32,
}
impl CustomIntAttribute {
    pub fn new(name: &str, default: u32) -> Self {
        Self {
            name: name.into(),
            value: default,
        }
    }
}
impl RouteAttribute for CustomIntAttribute {
    fn name(&self) -> &str {
        &self.name
    }
    fn attr_type(&self) -> FilterType {
        FilterType::Int
    }
    fn read(&self) -> FilterValue {
        FilterValue::Int(self.value)
    }
    fn write(&mut self, v: FilterValue) -> Result<(), String> {
        match v {
            FilterValue::Int(n) => {
                self.value = n;
                Ok(())
            }
            _ => Err("type mismatch".into()),
        }
    }
    fn is_read_only(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
pub struct CustomStringAttribute {
    name: String,
    value: String,
}
impl CustomStringAttribute {
    pub fn new(name: &str, default: String) -> Self {
        Self {
            name: name.into(),
            value: default,
        }
    }
}
impl RouteAttribute for CustomStringAttribute {
    fn name(&self) -> &str {
        &self.name
    }
    fn attr_type(&self) -> FilterType {
        FilterType::String
    }
    fn read(&self) -> FilterValue {
        FilterValue::String(self.value.clone())
    }
    fn write(&mut self, v: FilterValue) -> Result<(), String> {
        match v {
            FilterValue::String(s) => {
                self.value = s;
                Ok(())
            }
            _ => Err("type mismatch".into()),
        }
    }
    fn is_read_only(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
pub struct ReadOnlyAttribute {
    name: String,
    value: FilterValue,
}
impl ReadOnlyAttribute {
    pub fn new(name: &str, value: FilterValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}
impl RouteAttribute for ReadOnlyAttribute {
    fn name(&self) -> &str {
        &self.name
    }
    fn attr_type(&self) -> FilterType {
        self.value.type_of()
    }
    fn read(&self) -> FilterValue {
        self.value.clone()
    }
    fn write(&mut self, _v: FilterValue) -> Result<(), String> {
        Err("read-only".into())
    }
    fn is_read_only(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug)]
pub struct EnumAttribute {
    name: String,
    variants: Vec<String>,
    current: usize,
}
impl EnumAttribute {
    pub fn new(name: &str, variants: Vec<&str>, default_idx: usize) -> Self {
        Self {
            name: name.into(),
            variants: variants.iter().map(|s| s.to_string()).collect(),
            current: default_idx,
        }
    }
}
impl RouteAttribute for EnumAttribute {
    fn name(&self) -> &str {
        &self.name
    }
    fn attr_type(&self) -> FilterType {
        FilterType::Enum(crate::types::EnumType {
            name: self.name.clone(),
            values: self.variants.clone(),
        })
    }
    fn read(&self) -> FilterValue {
        FilterValue::Enum {
            type_name: self.name.clone(),
            variant: self.variants[self.current].clone(),
        }
    }
    fn write(&mut self, v: FilterValue) -> Result<(), String> {
        match v {
            FilterValue::Enum { variant, .. } => {
                match self.variants.iter().position(|s| s == &variant) {
                    Some(idx) => {
                        self.current = idx;
                        Ok(())
                    }
                    None => Err(format!("invalid variant: {variant}")),
                }
            }
            _ => Err("type mismatch".into()),
        }
    }
    fn is_read_only(&self) -> bool {
        false
    }
}

pub struct MplsAttributes;

impl MplsAttributes {
    pub fn register_all(registry: &mut AttributeRegistry) {
        registry.register(CustomIntAttribute::new("gw_mpls", 0));
        registry.register(CustomIntAttribute::new("mpls_label", 0));
        registry.register(EnumAttribute::new(
            "mpls_policy",
            vec![
                "MPLS_POLICY_NONE",
                "MPLS_POLICY_STATIC",
                "MPLS_POLICY_PREFIX",
                "MPLS_POLICY_AGGREGATE",
                "MPLS_POLICY_VRF",
            ],
            0,
        ));
        registry.register(CustomIntAttribute::new("mpls_class", 0));
    }
}
