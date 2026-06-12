use netpilot_filter::builtins::{from_hex, print, printn};
use netpilot_filter::value::FilterValue;
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn print_formats_multiple_values_with_newline() {
    let output = print(&[FilterValue::Int(42), FilterValue::String("hello".into())]);
    assert_eq!(output, "42 hello\n");
}

#[test]
fn printn_formats_without_newline() {
    assert_eq!(printn(&[FilterValue::Int(42)]), "42");
}

#[test]
fn print_bool() {
    assert_eq!(printn(&[FilterValue::Bool(true)]), "true");
}

#[test]
fn print_ip() {
    assert_eq!(
        printn(&[FilterValue::Ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)))]),
        "192.0.2.1"
    );
}

#[test]
fn from_hex_valid() {
    assert_eq!(from_hex("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
}

#[test]
fn from_hex_invalid() {
    assert!(from_hex("xyz").is_err());
}
