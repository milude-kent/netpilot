use crate::value::FilterValue;

pub fn defined(_attr_name: &str) -> bool {
    false
}

pub fn unset(_attr_name: &str) -> Result<(), String> {
    Err("not implemented".into())
}

pub fn print(_values: &[FilterValue]) {
    // placeholder
}

pub fn printn(_values: &[FilterValue]) {
    // placeholder
}

pub fn from_hex(hex_str: &str) -> Result<Vec<u8>, String> {
    hex::decode(hex_str).map_err(|e| format!("invalid hex: {e}"))
}
