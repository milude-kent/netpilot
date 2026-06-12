use routeplane_filter::Nettype;
use routeplane_filter::builtins;
use routeplane_filter::types::FilterType;
use routeplane_filter::value::{FilterValue, PrefixData, RouteDistinguisher};
use std::net::{IpAddr, Ipv4Addr};

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

// --- bgppath tests ---

use routeplane_filter::value::{AsPath, AsPathSegment};

#[test]
fn bgppath_construct_and_access() {
    let path = AsPath {
        segments: vec![
            AsPathSegment::AsSequence(vec![64500, 64501, 64502]),
            AsPathSegment::AsSet(vec![64510, 64511]),
        ],
    };

    // .first — first ASN in path
    assert_eq!(path.first(), Some(64500));

    // .last — last ASN in path
    assert_eq!(path.last(), Some(64511));

    // .last_nonaggregated — last ASN in last AS_SEQUENCE
    assert_eq!(path.last_nonaggregated(), Some(64502));

    // .len — total number of ASNs
    assert_eq!(path.len(), 5);

    // .empty
    let empty_path = AsPath { segments: vec![] };
    assert!(empty_path.empty());
    assert!(!path.empty());
}

#[test]
fn bgppath_prepend() {
    let mut path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500])],
    };
    path.prepend(64999);
    assert_eq!(path.first(), Some(64999));
    assert_eq!(path.len(), 2);
}

#[test]
fn bgppath_delete_removes_asn() {
    let mut path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64501, 64502])],
    };
    path.delete(64501);
    assert_eq!(path.len(), 2);
    // 64501 should be removed
    let all_asns: Vec<u32> = path
        .segments
        .iter()
        .flat_map(|s| s.asns().to_vec())
        .collect();
    assert!(!all_asns.contains(&64501));
}

#[test]
fn bgppath_filter_keeps_matching() {
    let mut path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64501, 64502])],
    };
    // filter keeps only ASNs > 64500
    path.filter(|asn| *asn > 64500);
    assert_eq!(path.len(), 2);
}

#[test]
fn bgppath_prepend_on_empty_path() {
    let mut path = AsPath { segments: vec![] };
    path.prepend(64500);
    assert_eq!(path.first(), Some(64500));
    assert_eq!(path.len(), 1);
}

// --- bgpmask tests ---

use routeplane_filter::value::{AsMaskPattern, AsPathMask};

#[test]
fn bgpmask_matches_empty_path() {
    let mask = AsPathMask { patterns: vec![] };
    let path = AsPath { segments: vec![] };
    assert!(mask.matches(&path));
}

#[test]
fn bgpmask_matches_exact_sequence() {
    let mask = AsPathMask {
        patterns: vec![AsMaskPattern::Exact(64500), AsMaskPattern::Exact(64501)],
    };
    let path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64501])],
    };
    assert!(mask.matches(&path));
}

#[test]
fn bgpmask_any_matches_single() {
    let mask = AsPathMask {
        patterns: vec![AsMaskPattern::Any, AsMaskPattern::Exact(64500)],
    };
    let path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64999, 64500])],
    };
    assert!(mask.matches(&path));
}

#[test]
fn bgpmask_one_or_more_matches_multiple() {
    let mask = AsPathMask {
        patterns: vec![AsMaskPattern::OneOrMore],
    };
    let path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64501])],
    };
    assert!(mask.matches(&path));

    let empty_path = AsPath { segments: vec![] };
    assert!(!mask.matches(&empty_path));
}

#[test]
fn bgpmask_any_optional_skips() {
    let mask = AsPathMask {
        patterns: vec![
            AsMaskPattern::Exact(64500),
            AsMaskPattern::AnyOptional,
            AsMaskPattern::Exact(64502),
        ],
    };
    // matches 64500 64502 directly (skip middle)
    let path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64502])],
    };
    assert!(mask.matches(&path));

    // matches 64500 64999 64502 (with middle)
    let path2 = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64999, 64502])],
    };
    assert!(mask.matches(&path2));
}

#[test]
fn bgpmask_set_matches() {
    let mask = AsPathMask {
        patterns: vec![AsMaskPattern::Set(vec![64500, 64501, 64502])],
    };
    let path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64501])],
    };
    assert!(mask.matches(&path));
    let path2 = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64999])],
    };
    assert!(!mask.matches(&path2));
}

#[test]
fn bgpmask_range_matches() {
    let mask = AsPathMask {
        patterns: vec![AsMaskPattern::Range(64500, 64510)],
    };
    let path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64505])],
    };
    assert!(mask.matches(&path));
    let path2 = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64511])],
    };
    assert!(!mask.matches(&path2));
}

// --- Community list tests (#272) ---

use routeplane_filter::value::{ClistEntry, EcValue, LcValue};
use routeplane_filter::value::{
    clist_add, clist_delete, clist_filter, clist_max, clist_min, eclist_add, eclist_delete,
    eclist_filter, eclist_min, lclist_add, lclist_delete, lclist_filter, lclist_min,
};

#[test]
fn clist_operations() {
    let mut clist: Vec<ClistEntry> = vec![(64500, 100), (64500, 200)];

    assert_eq!(clist.len(), 2);

    // .add(p) — no duplicates
    clist_add(&mut clist, (64500, 300));
    assert_eq!(clist.len(), 3);
    clist_add(&mut clist, (64500, 300)); // duplicate
    assert_eq!(clist.len(), 3);

    // .delete(p)
    clist_delete(&mut clist, &(64500, 100));
    assert_eq!(clist.len(), 2);

    // .filter(p)
    clist_filter(&mut clist, |(asn, _)| *asn == 64500);
    assert_eq!(clist.len(), 2);

    // .min
    assert_eq!(clist_min(&clist), Some((64500, 200)));

    // .max
    assert_eq!(clist_max(&clist), Some((64500, 300)));
}

#[test]
fn eclist_operations() {
    let mut eclist: Vec<EcValue> = vec![
        EcValue {
            kind: 2,
            key: 0,
            value: 100,
        },
        EcValue {
            kind: 2,
            key: 0,
            value: 200,
        },
    ];

    assert_eq!(eclist.len(), 2);

    eclist_add(
        &mut eclist,
        EcValue {
            kind: 2,
            key: 1,
            value: 300,
        },
    );
    assert_eq!(eclist.len(), 3);

    eclist_delete(
        &mut eclist,
        &EcValue {
            kind: 2,
            key: 0,
            value: 100,
        },
    );
    assert_eq!(eclist.len(), 2);

    eclist_filter(&mut eclist, |ec| ec.key == 0);
    assert_eq!(eclist.len(), 1);

    let min_val = eclist_min(&eclist);
    assert!(min_val.is_some());
}

#[test]
fn lclist_operations() {
    let mut lclist: Vec<LcValue> = vec![
        LcValue {
            asn: 64500,
            data1: 1,
            data2: 100,
        },
        LcValue {
            asn: 64500,
            data1: 1,
            data2: 200,
        },
    ];

    assert_eq!(lclist.len(), 2);

    lclist_add(
        &mut lclist,
        LcValue {
            asn: 64500,
            data1: 1,
            data2: 300,
        },
    );
    assert_eq!(lclist.len(), 3);

    lclist_delete(
        &mut lclist,
        &LcValue {
            asn: 64500,
            data1: 1,
            data2: 100,
        },
    );
    assert_eq!(lclist.len(), 2);

    lclist_filter(&mut lclist, |lc| lc.data1 == 1);
    assert_eq!(lclist.len(), 2);

    let min_val = lclist_min(&lclist);
    assert!(min_val.is_some());
}

#[test]
fn clist_empty_operations() {
    let mut clist: Vec<ClistEntry> = vec![];
    assert_eq!(clist_min(&clist), None);
    assert_eq!(clist_max(&clist), None);
    clist_add(&mut clist, (64500, 100));
    assert_eq!(clist.len(), 1);
}

// --- Bytestring, MAC, and Route-Distinguisher tests (#273, #274, #275) ---

#[test]
fn bytestring_from_hex_valid() {
    let bs = routeplane_filter::builtins::from_hex("0102ff").expect("valid hex");
    assert_eq!(bs, vec![0x01, 0x02, 0xff]);
}

#[test]
fn bytestring_from_hex_invalid() {
    assert!(routeplane_filter::builtins::from_hex("xyz").is_err());
}

#[test]
fn bytestring_concat() {
    let a = vec![0x01, 0x02];
    let b = vec![0x03, 0x04];
    let c: Vec<u8> = [a.as_slice(), b.as_slice()].concat();
    assert_eq!(c, vec![0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn mac_value_roundtrip() {
    let mac: [u8; 6] = [0x62, 0x68, 0x7f, 0xd9, 0xc6, 0xec];
    let fv = FilterValue::Mac(mac);
    assert_eq!(fv.type_of(), FilterType::Mac);
}

#[test]
fn rd_type0_format() {
    let rd = RouteDistinguisher::Type0 {
        admin: 64500,
        assigned: 100,
    };
    let fv = FilterValue::Rd(rd);
    assert_eq!(fv.type_of(), FilterType::Rd);
}

#[test]
fn rd_type1_format() {
    let rd = RouteDistinguisher::Type1 {
        ip: Ipv4Addr::new(192, 0, 2, 1),
        assigned: 100,
    };
    assert_eq!(FilterValue::Rd(rd).type_of(), FilterType::Rd);
}

#[test]
fn rd_type2_format() {
    let rd = RouteDistinguisher::Type2 {
        asn: 64500,
        assigned: 100,
    };
    assert_eq!(FilterValue::Rd(rd).type_of(), FilterType::Rd);
}

#[test]
fn prefix_with_rd_field() {
    let prefix = PrefixData {
        nettype: Nettype::Vpn4,
        ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)),
        length: 24,
        rd: Some(RouteDistinguisher::Type2 {
            asn: 64500,
            assigned: 100,
        }),
        source_ip: None,
        source_length: None,
        maxlen: None,
        asn: None,
        mac: None,
        vlan_id: None,
        evpn_type: None,
        evpn_tag: None,
        evpn_esi: None,
        router_ip: None,
    };
    assert!(prefix.rd.is_some());
}
