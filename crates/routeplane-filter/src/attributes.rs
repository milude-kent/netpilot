use crate::{types::FilterType, value::FilterValue};

pub trait RouteAttribute {
    fn name(&self) -> &str;
    fn attr_type(&self) -> FilterType;
    fn read(&self) -> FilterValue;
    fn write(&mut self, value: FilterValue) -> Result<(), String>;
    fn is_read_only(&self) -> bool;
}
