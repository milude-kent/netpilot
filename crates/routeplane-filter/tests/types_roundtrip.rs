use routeplane_filter::types::FilterType;
use routeplane_filter::value::FilterValue;
use routeplane_filter::Nettype;
use routeplane_filter::builtins;

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

// --- Nettype::from_name tests for all 18 variants ---

#[test]
fn nettype_from_name_ip4() {
    assert_eq!(Nettype::from_name("NET_IP4"), Some(Nettype::Ip4));
}

#[test]
fn nettype_from_name_ip6() {
    assert_eq!(Nettype::from_name("NET_IP6"), Some(Nettype::Ip6));
}

#[test]
fn nettype_from_name_ip6_sadr() {
    assert_eq!(Nettype::from_name("NET_IP6_SADR"), Some(Nettype::Ip6Sadr));
}

#[test]
fn nettype_from_name_vpn4() {
    assert_eq!(Nettype::from_name("NET_VPN4"), Some(Nettype::Vpn4));
}

#[test]
fn nettype_from_name_vpn6() {
    assert_eq!(Nettype::from_name("NET_VPN6"), Some(Nettype::Vpn6));
}

#[test]
fn nettype_from_name_roa4() {
    assert_eq!(Nettype::from_name("NET_ROA4"), Some(Nettype::Roa4));
}

#[test]
fn nettype_from_name_roa6() {
    assert_eq!(Nettype::from_name("NET_ROA6"), Some(Nettype::Roa6));
}

#[test]
fn nettype_from_name_aspa() {
    assert_eq!(Nettype::from_name("NET_ASPA"), Some(Nettype::Aspa));
}

#[test]
fn nettype_from_name_flow4() {
    assert_eq!(Nettype::from_name("NET_FLOW4"), Some(Nettype::Flow4));
}

#[test]
fn nettype_from_name_flow6() {
    assert_eq!(Nettype::from_name("NET_FLOW6"), Some(Nettype::Flow6));
}

#[test]
fn nettype_from_name_eth() {
    assert_eq!(Nettype::from_name("NET_ETH"), Some(Nettype::Eth));
}

#[test]
fn nettype_from_name_mpls() {
    assert_eq!(Nettype::from_name("NET_MPLS"), Some(Nettype::Mpls));
}

#[test]
fn nettype_from_name_evpn() {
    assert_eq!(Nettype::from_name("NET_EVPN"), Some(Nettype::Evpn));
}

#[test]
fn nettype_from_name_evpn_ead() {
    assert_eq!(Nettype::from_name("NET_EVPN_EAD"), Some(Nettype::EvpnEad));
}

#[test]
fn nettype_from_name_evpn_mac() {
    assert_eq!(Nettype::from_name("NET_EVPN_MAC"), Some(Nettype::EvpnMac));
}

#[test]
fn nettype_from_name_evpn_imet() {
    assert_eq!(Nettype::from_name("NET_EVPN_IMET"), Some(Nettype::EvpnImet));
}

#[test]
fn nettype_from_name_evpn_es() {
    assert_eq!(Nettype::from_name("NET_EVPN_ES"), Some(Nettype::EvpnEs));
}

#[test]
fn nettype_from_name_neighbor() {
    assert_eq!(Nettype::from_name("NET_NEIGHBOR"), Some(Nettype::Neighbor));
}

#[test]
fn nettype_from_name_unknown() {
    assert_eq!(Nettype::from_name("NET_UNKNOWN"), None);
    assert_eq!(Nettype::from_name(""), None);
    assert_eq!(Nettype::from_name("garbage"), None);
}

// --- builtins::from_hex tests ---

#[test]
fn from_hex_valid() {
    let result = builtins::from_hex("deadbeef");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
}

#[test]
fn from_hex_valid_empty() {
    let result = builtins::from_hex("");
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn from_hex_invalid() {
    let result = builtins::from_hex("xyz");
    assert!(result.is_err());
}

#[test]
fn from_hex_odd_length() {
    let result = builtins::from_hex("abc");
    assert!(result.is_err());
}

// --- FilterValue construction + type_of() roundtrip tests ---

#[test]
fn filter_value_bool_roundtrip() {
    let v = FilterValue::Bool(true);
    assert_eq!(v.type_of(), FilterType::Bool);
    let v = FilterValue::Bool(false);
    assert_eq!(v.type_of(), FilterType::Bool);
}

#[test]
fn filter_value_prefix_roundtrip() {
    use routeplane_filter::value::PrefixData;
    let pd = PrefixData {
        nettype: Nettype::Ip4,
        ip: "10.0.0.0".parse().unwrap(),
        length: 8,
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
    };
    let v = FilterValue::Prefix(pd);
    assert_eq!(v.type_of(), FilterType::Prefix);
}

#[test]
fn filter_value_ec_roundtrip() {
    use routeplane_filter::value::EcValue;
    let ev = EcValue {
        kind: 1,
        key: 100,
        value: 200,
    };
    let v = FilterValue::Ec(ev);
    assert_eq!(v.type_of(), FilterType::Ec);
}

#[test]
fn filter_value_mac_roundtrip() {
    let v = FilterValue::Mac([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    assert_eq!(v.type_of(), FilterType::Mac);
}

#[test]
fn filter_value_ip_roundtrip() {
    let v = FilterValue::Ip("192.168.1.1".parse().unwrap());
    assert_eq!(v.type_of(), FilterType::Ip);

    let v6 = FilterValue::Ip("::1".parse().unwrap());
    assert_eq!(v6.type_of(), FilterType::Ip);
}
