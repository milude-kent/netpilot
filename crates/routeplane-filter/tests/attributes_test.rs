use routeplane_filter::attributes::{AttributeRegistry, RouteAttribute};
use routeplane_filter::builtins::{defined, unset};
use routeplane_filter::types::FilterType;
use routeplane_filter::value::FilterValue;

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
