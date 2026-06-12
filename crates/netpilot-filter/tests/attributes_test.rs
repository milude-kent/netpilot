use netpilot_filter::attributes::{
    AttributeRegistry, CustomIntAttribute, MplsAttributes, RouteAttribute,
};
use netpilot_filter::builtins::{defined, unset};
use netpilot_filter::types::FilterType;
use netpilot_filter::value::FilterValue;

struct TestAttr {
    value: FilterValue,
    read_only: bool,
}
impl RouteAttribute for TestAttr {
    fn name(&self) -> &str {
        "test_attr"
    }
    fn attr_type(&self) -> FilterType {
        FilterType::Int
    }
    fn read(&self) -> FilterValue {
        self.value.clone()
    }
    fn write(&mut self, v: FilterValue) -> Result<(), String> {
        if self.read_only {
            return Err("ro".into());
        }
        self.value = v;
        Ok(())
    }
    fn is_read_only(&self) -> bool {
        self.read_only
    }
}

#[test]
fn defined_true_for_registered() {
    let mut r = AttributeRegistry::new();
    r.register(TestAttr {
        value: FilterValue::Int(42),
        read_only: false,
    });
    assert!(defined(&r, "test_attr"));
}

#[test]
fn defined_false_for_missing() {
    assert!(!defined(&AttributeRegistry::new(), "nonexistent"));
}

#[test]
fn unset_clears_mutable() {
    let mut r = AttributeRegistry::new();
    r.register(TestAttr {
        value: FilterValue::Int(42),
        read_only: false,
    });
    unset(&mut r, "test_attr").unwrap();
    assert_eq!(r.read("test_attr").unwrap(), FilterValue::Int(0));
}

#[test]
fn unset_fails_readonly() {
    let mut r = AttributeRegistry::new();
    r.register(TestAttr {
        value: FilterValue::Int(42),
        read_only: true,
    });
    assert!(unset(&mut r, "test_attr").is_err());
}

#[test]
fn unset_fails_missing() {
    assert!(unset(&mut AttributeRegistry::new(), "nonexistent").is_err());
}

#[test]
fn gw_mpls_attribute() {
    let mut r = AttributeRegistry::new();
    MplsAttributes::register_all(&mut r);
    r.write("gw_mpls", FilterValue::Int(1000)).unwrap();
    assert_eq!(r.read("gw_mpls").unwrap(), FilterValue::Int(1000));
}

#[test]
fn mpls_label_attribute() {
    let mut r = AttributeRegistry::new();
    MplsAttributes::register_all(&mut r);
    r.write("mpls_label", FilterValue::Int(2000)).unwrap();
    assert_eq!(r.read("mpls_label").unwrap(), FilterValue::Int(2000));
}

#[test]
fn mpls_policy_default_is_none() {
    let mut r = AttributeRegistry::new();
    MplsAttributes::register_all(&mut r);
    let val = r.read("mpls_policy").unwrap();
    assert!(matches!(&val, FilterValue::Enum { variant, .. } if variant == "MPLS_POLICY_NONE"));
}

#[test]
fn mpls_policy_set_to_prefix() {
    let mut r = AttributeRegistry::new();
    MplsAttributes::register_all(&mut r);
    r.write(
        "mpls_policy",
        FilterValue::Enum {
            type_name: "mpls_policy".into(),
            variant: "MPLS_POLICY_PREFIX".into(),
        },
    )
    .unwrap();
    let val = r.read("mpls_policy").unwrap();
    assert!(matches!(&val, FilterValue::Enum { variant, .. } if variant == "MPLS_POLICY_PREFIX"));
}

#[test]
fn mpls_class_attribute() {
    let mut r = AttributeRegistry::new();
    MplsAttributes::register_all(&mut r);
    r.write("mpls_class", FilterValue::Int(5)).unwrap();
    assert_eq!(r.read("mpls_class").unwrap(), FilterValue::Int(5));
}

#[test]
fn igp_metric_attribute() {
    let mut r = AttributeRegistry::new();
    r.register(CustomIntAttribute::new("igp_metric", 0));
    r.write("igp_metric", FilterValue::Int(100)).unwrap();
    assert_eq!(r.read("igp_metric").unwrap(), FilterValue::Int(100));
}

#[test]
fn evpn_mac_prefix_operators() {
    use netpilot_filter::nettype::Nettype;
    use netpilot_filter::value::PrefixData;
    use std::net::{IpAddr, Ipv4Addr};
    let prefix = PrefixData {
        nettype: Nettype::EvpnMac,
        ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        length: 32,
        mac: Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]),
        vlan_id: Some(100),
        evpn_type: Some(2),
        evpn_tag: Some(200),
        evpn_esi: Some([0x00; 10]),
        router_ip: Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))),
        rd: None,
        source_ip: None,
        source_length: None,
        maxlen: None,
        asn: None,
    };
    assert_eq!(prefix.evpn_type, Some(2));
    assert_eq!(prefix.mac, Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]));
    assert_eq!(prefix.evpn_tag, Some(200));
    assert_eq!(
        prefix.router_ip,
        Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)))
    );
}
