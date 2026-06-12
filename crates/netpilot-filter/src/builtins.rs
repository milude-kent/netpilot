use crate::attributes::AttributeRegistry;
use crate::value::FilterValue;

pub fn defined(registry: &AttributeRegistry, attr_name: &str) -> bool {
    registry.is_defined(attr_name)
}

pub fn unset(registry: &mut AttributeRegistry, attr_name: &str) -> Result<(), String> {
    registry.unset(attr_name)
}

pub fn print(values: &[FilterValue]) -> String {
    let mut out = String::new();
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&v.to_string());
    }
    out.push('\n');
    out
}

pub fn printn(values: &[FilterValue]) -> String {
    let mut out = String::new();
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&v.to_string());
    }
    out
}

pub fn from_hex(hex_str: &str) -> Result<Vec<u8>, String> {
    hex::decode(hex_str).map_err(|e| format!("invalid hex: {e}"))
}
