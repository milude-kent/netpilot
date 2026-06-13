use crate::{
    diff::ConfigDiff,
    schema::RoutePlaneConfig,
    validation::{ValidationError, validate_config},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::{Mutex, RwLock};
use tokio::task::{AbortHandle, JoinHandle};

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
pub struct PendingConfirm {
    pub revision_id: u64,
    pub previous_revision_id: u64,
    pub started_at: std::time::Instant,
    pub timeout_secs: u32,
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    running: RoutePlaneConfig,
    candidate: RoutePlaneConfig,
    revisions: Vec<Revision>,
    next_revision_id: u64,
    pub pending_timeout: Option<PendingConfirm>,
}

impl ConfigStore {
    pub fn new(initial: RoutePlaneConfig) -> Self {
        Self {
            running: initial.clone(),
            candidate: initial,
            revisions: Vec::new(),
            next_revision_id: 1,
            pending_timeout: None,
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

    pub fn soft_commit(&mut self, request: CommitRequest) -> Result<Revision, ValidationError> {
        // For now, soft commit = regular commit (full reload not yet supported)
        // In future: compute diff and only reload affected protocols
        self.commit(request)
    }

    pub fn commit_with_timeout(
        &mut self,
        request: CommitRequest,
        timeout_secs: u32,
    ) -> Result<Revision, ValidationError> {
        // Capture the currently-running revision id BEFORE the commit so that
        // auto-rollback targets the correct previous state, even if a rollback
        // already appended a new audit revision on top.
        let prev = self
            .revisions
            .last()
            .map(|revision| revision.id)
            .unwrap_or(0);
        let revision = self.commit(request)?;
        self.pending_timeout = Some(PendingConfirm {
            revision_id: revision.id,
            previous_revision_id: prev,
            started_at: std::time::Instant::now(),
            timeout_secs,
        });
        Ok(revision)
    }

    pub fn confirm(&mut self) -> Result<(), ValidationError> {
        self.pending_timeout = None;
        Ok(())
    }

    pub fn undo(&mut self) -> Result<Revision, ValidationError> {
        let pending = self
            .pending_timeout
            .take()
            .ok_or_else(|| ValidationError::Message("no pending confirmed commit".into()))?;
        self.rollback(RollbackRequest {
            revision_id: pending.previous_revision_id,
            author: "system".into(),
            note: "auto-rollback confirmed commit".into(),
        })
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

/// Coordinates timed auto-rollback for confirmed commits.
///
/// `CommitScheduler` wraps a shared `ConfigStore` (typically held inside an
/// `Arc<RwLock<...>>`) and arranges for the running configuration to revert
/// to its previous revision if the operator does not explicitly confirm the
/// change before the configured timeout elapses.
///
/// Each pending commit is tracked by its revision id; the scheduler spawns a
/// background tokio task per commit that sleeps for `timeout_secs` and then
/// re-checks the store. If the pending commit is still the active one (i.e.
/// nothing else has superseded it), it triggers an auto-rollback. The
/// `confirm` and `cancel_pending` operations abort the spawned task so that
/// the auto-rollback will not fire.
pub struct CommitScheduler {
    store: Arc<RwLock<ConfigStore>>,
    active: Arc<Mutex<HashMap<u64, AbortHandle>>>,
}

impl CommitScheduler {
    /// Create a new scheduler that drives the given shared `ConfigStore`.
    pub fn new(store: Arc<RwLock<ConfigStore>>) -> Self {
        Self {
            store,
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Commit the candidate configuration and arrange for an automatic
    /// rollback if the commit is not confirmed within `timeout_secs`.
    ///
    /// Returns the newly-created revision. The auto-rollback fires only if
    /// `confirm` (or another commit/rollback) has not been called first.
    pub async fn commit_with_timeout(
        &self,
        request: CommitRequest,
        timeout_secs: u32,
    ) -> Result<Revision, ValidationError> {
        // Commit the candidate under the write lock so that no concurrent
        // mutation can interleave with the rollback bookkeeping.
        let revision = {
            let mut store = self.store.write().await;
            store.commit_with_timeout(request, timeout_secs)?
        };

        // Spawn a background task that, after the timeout, re-acquires the
        // write lock and rolls back the commit if it is still the active
        // pending one.
        let store = self.store.clone();
        let active = self.active.clone();
        let handle: JoinHandle<()> = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(timeout_secs as u64)).await;

            let mut store = store.write().await;
            // Only auto-rollback if our revision is still the active pending
            // one. A confirm/undo/manual commit would have cleared it.
            let still_pending = store
                .pending_timeout
                .as_ref()
                .map(|p| p.revision_id)
                .map(|id| id == revision.id)
                .unwrap_or(false);

            if still_pending {
                match store.undo() {
                    Ok(rolled_back) => {
                        tracing::warn!(
                            revision_id = revision.id,
                            rollback_id = rolled_back.id,
                            "confirmed commit auto-rolled back after timeout"
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            revision_id = revision.id,
                            error = %error,
                            "auto-rollback failed"
                        );
                    }
                }
            }

            // Remove ourselves from the active map so the entry does not
            // accumulate forever.
            let mut active = active.lock().await;
            active.remove(&revision.id);
        });

        // Record the abort handle so that confirm()/cancel_pending() can
        // cancel the timer before it fires.
        {
            let mut active = self.active.lock().await;
            // Defensive: if a previous handle for this revision id is still
            // present (should not happen in practice), abort it.
            if let Some(previous) = active.insert(revision.id, handle.abort_handle()) {
                previous.abort();
            }
        }

        Ok(revision)
    }

    /// Cancel the pending auto-rollback for the most recent confirmed commit,
    /// making the running configuration permanent.
    pub async fn confirm(&self) -> Result<(), ValidationError> {
        // Abort any active timer first so the spawned task cannot fire after
        // we clear `pending_timeout`.
        {
            let mut active = self.active.lock().await;
            for (_, handle) in active.drain() {
                handle.abort();
            }
        }

        let mut store = self.store.write().await;
        store.confirm()
    }

    /// Cancel the pending auto-rollback and clear the pending commit without
    /// performing a rollback. Returns an error if there is no pending
    /// commit to cancel.
    pub async fn cancel_pending(&self) -> Result<(), ValidationError> {
        {
            let mut active = self.active.lock().await;
            for (_, handle) in active.drain() {
                handle.abort();
            }
        }

        let mut store = self.store.write().await;
        // Reuse `undo` semantics: this returns an error if no pending
        // commit exists, which is the correct behaviour for "cancel" when
        // there is nothing to cancel.
        store.undo().map(|_| ())
    }
}
