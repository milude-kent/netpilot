use crate::schema::RoutePlaneConfig;

#[derive(Clone, Debug)]
pub struct CommitRequest {
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct RollbackRequest {
    pub revision_id: u64,
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Revision {
    pub id: u64,
    pub config: RoutePlaneConfig,
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    running: RoutePlaneConfig,
    candidate: RoutePlaneConfig,
    revisions: Vec<Revision>,
}
