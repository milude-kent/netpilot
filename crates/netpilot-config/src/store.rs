use crate::{
    diff::ConfigDiff,
    schema::RoutePlaneConfig,
    validation::{ValidationError, validate_config},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommitRequest {
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RollbackRequest {
    pub revision_id: u64,
    pub author: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Revision {
    pub id: u64,
    pub config: RoutePlaneConfig,
    pub author: String,
    pub note: String,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    running: RoutePlaneConfig,
    candidate: RoutePlaneConfig,
    revisions: Vec<Revision>,
    next_revision_id: u64,
}

impl ConfigStore {
    pub fn new(initial: RoutePlaneConfig) -> Self {
        Self {
            running: initial.clone(),
            candidate: initial,
            revisions: Vec::new(),
            next_revision_id: 1,
        }
    }

    pub fn running(&self) -> &RoutePlaneConfig {
        &self.running
    }

    pub fn candidate(&self) -> &RoutePlaneConfig {
        &self.candidate
    }

    pub fn revisions(&self) -> &[Revision] {
        &self.revisions
    }

    pub fn replace_candidate(
        &mut self,
        candidate: RoutePlaneConfig,
    ) -> Result<(), ValidationError> {
        validate_config(&candidate)?;
        self.candidate = candidate;
        Ok(())
    }

    pub fn diff(&self) -> ConfigDiff {
        ConfigDiff::between(&self.running, &self.candidate)
    }

    pub fn commit(&mut self, request: CommitRequest) -> Result<Revision, ValidationError> {
        validate_config(&self.candidate)?;
        self.running = self.candidate.clone();
        let revision = self.create_revision(request.author, request.note, self.running.clone());
        self.revisions.push(revision.clone());
        Ok(revision)
    }

    pub fn rollback(&mut self, request: RollbackRequest) -> Result<Revision, ValidationError> {
        let target = self
            .revisions
            .iter()
            .find(|revision| revision.id == request.revision_id)
            .map(|revision| revision.config.clone())
            .ok_or_else(|| {
                ValidationError::Message(format!("revision {} does not exist", request.revision_id))
            })?;

        validate_config(&target)?;
        self.running = target.clone();
        self.candidate = target.clone();
        let revision = self.create_revision(request.author, request.note, target);
        self.revisions.push(revision.clone());
        Ok(revision)
    }

    fn create_revision(
        &mut self,
        author: String,
        note: String,
        config: RoutePlaneConfig,
    ) -> Revision {
        let revision = Revision {
            id: self.next_revision_id,
            config,
            author,
            note,
            created_at: OffsetDateTime::now_utc(),
        };
        self.next_revision_id += 1;
        revision
    }
}
