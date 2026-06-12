use routeplane_filter::types::FilterType;
use routeplane_filter::value::FilterValue;

#[test]
fn bool_type_exists() {
    let t = FilterType::Bool;
    assert_eq!(t.to_string(), "bool");
    let v = FilterValue::Bool(true);
    assert_eq!(v.type_of(), FilterType::Bool);
}

#[test]
fn int_type_exists() {
    let t = FilterType::Int;
    assert_eq!(t.to_string(), "int");
    let v = FilterValue::Int(42);
    assert_eq!(v.type_of(), FilterType::Int);
}

#[test]
fn ip_type_exists() {
    let t = FilterType::Ip;
    assert_eq!(t.to_string(), "ip");
}

#[test]
fn prefix_type_exists() {
    let t = FilterType::Prefix;
    assert_eq!(t.to_string(), "prefix");
}

#[test]
fn string_type_exists() {
    let t = FilterType::String;
    assert_eq!(t.to_string(), "string");
}
