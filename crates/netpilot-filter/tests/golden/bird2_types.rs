use netpilot_filter::{
    builtins::{from_hex, print, printn},
    nettype::Nettype,
    value::{
        AsMaskPattern, AsPath, AsPathMask, AsPathSegment, EcValue, FilterValue, LcValue,
        PrefixData, RouteDistinguisher,
    },
};
use std::net::{IpAddr, Ipv4Addr};

// --- bgppath golden tests ---

#[test]
fn golden_bgppath_first_returns_first_asn() {
    let path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64501, 64502])],
    };
    assert_eq!(path.first(), Some(64500));
    assert_eq!(path.last(), Some(64502));
}

#[test]
fn golden_bgppath_prepend_adds_to_front() {
    let mut path = AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64501])],
    };
    path.prepend(64500);
    assert_eq!(path.first(), Some(64500));
    assert_eq!(path.len(), 2);
}

#[test]
fn golden_bgppath_empty_is_true_for_no_asns() {
    assert!(AsPath { segments: vec![] }.empty());
}

// --- bgpmask golden tests ---

#[test]
fn golden_bgpmask_asterisk_matches_any_single_asn() {
    let mask = AsPathMask {
        patterns: vec![AsMaskPattern::Any, AsMaskPattern::Exact(64500)],
    };
    assert!(mask.matches(&AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64999, 64500])]
    }));
    assert!(!mask.matches(&AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64999, 64998])]
    }));
}

#[test]
fn golden_bgpmask_question_is_optional() {
    let mask = AsPathMask {
        patterns: vec![
            AsMaskPattern::Exact(64500),
            AsMaskPattern::AnyOptional,
            AsMaskPattern::Exact(64502),
        ],
    };
    assert!(mask.matches(&AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64502])]
    }));
    assert!(mask.matches(&AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64999, 64502])]
    }));
}

#[test]
fn golden_bgpmask_plus_matches_one_or_more() {
    let mask = AsPathMask {
        patterns: vec![AsMaskPattern::Exact(64500), AsMaskPattern::OneOrMore],
    };
    assert!(mask.matches(&AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64501])]
    }));
    assert!(mask.matches(&AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500, 64501, 64502])]
    }));
    assert!(!mask.matches(&AsPath {
        segments: vec![AsPathSegment::AsSequence(vec![64500])]
    }));
}

// --- clist golden tests ---

#[test]
fn golden_clist_no_duplicates() {
    let mut clist: Vec<(u16, u16)> = vec![(64500, 100)];
    netpilot_filter::value::clist_add(&mut clist, (64500, 100));
    assert_eq!(clist.len(), 1);
}

// --- from_hex golden tests ---

#[test]
fn golden_from_hex_matches_bird2_behavior() {
    assert_eq!(from_hex("0102").unwrap(), vec![0x01, 0x02]);
}

// --- rd golden tests ---

#[test]
fn golden_rd_type0_display() {
    let rd = RouteDistinguisher::Type0 {
        admin: 64500,
        assigned: 100,
    };
    assert_eq!(format!("{}", FilterValue::Rd(rd)), "64500:100");
}

#[test]
fn golden_rd_type1_display() {
    let rd = RouteDistinguisher::Type1 {
        ip: Ipv4Addr::new(192, 0, 2, 1),
        assigned: 100,
    };
    assert_eq!(format!("{}", FilterValue::Rd(rd)), "192.0.2.1:100");
}

#[test]
fn golden_rd_type2_display() {
    let rd = RouteDistinguisher::Type2 {
        asn: 64500,
        assigned: 100,
    };
    assert_eq!(format!("{}", FilterValue::Rd(rd)), "64500:100");
}

// --- community display golden tests ---

#[test]
fn golden_ec_display() {
    let ec = EcValue {
        kind: 2,
        key: 0,
        value: 64500,
    };
    assert_eq!(format!("{}", FilterValue::Ec(ec)), "(2,0,64500)");
}

#[test]
fn golden_lc_display() {
    let lc = LcValue {
        asn: 64500,
        data1: 1,
        data2: 100,
    };
    assert_eq!(format!("{}", FilterValue::Lc(lc)), "(64500,1,100)");
}

// --- print golden tests ---

#[test]
fn golden_print_with_newline() {
    assert_eq!(
        print(&[FilterValue::String("route".into()), FilterValue::Int(42)]),
        "route 42\n"
    );
}

#[test]
fn golden_printn_without_newline() {
    assert_eq!(printn(&[FilterValue::Int(42)]), "42");
}

// --- EVPN golden test ---

#[test]
fn golden_evpn_mac_prefix() {
    let prefix = PrefixData {
        nettype: Nettype::EvpnMac,
        ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        length: 32,
        mac: Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]),
        evpn_type: Some(2),
        evpn_tag: Some(100),
        evpn_esi: Some([0x01; 10]),
        router_ip: Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))),
        rd: None,
        source_ip: None,
        source_length: None,
        maxlen: None,
        asn: None,
        vlan_id: None,
    };
    assert_eq!(prefix.evpn_type, Some(2));
    assert_eq!(prefix.mac, Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]));
    assert_eq!(prefix.evpn_tag, Some(100));
    assert_eq!(
        prefix.router_ip,
        Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)))
    );
}
