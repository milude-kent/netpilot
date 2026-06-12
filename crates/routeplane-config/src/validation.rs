use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("{0}")]
    Message(String),
}
