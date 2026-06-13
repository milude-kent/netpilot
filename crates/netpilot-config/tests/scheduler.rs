//! Tests for `CommitScheduler` — the async auto-rollback coordinator that
//! drives timed `commit-confirmed` operations.
//!
//! The scheduler wraps a shared `ConfigStore` (held inside an
//! `Arc<RwLock<...>>`) and spawns a tokio task per pending commit that
//! auto-rolls back the change if the operator does not confirm in time.

use std::sync::Arc;
use std::time::Duration;

use netpilot_config::{
    AddressFamily, CommitRequest, CommitScheduler, ConfigStore, ProtocolConfig, RoutePlaneConfig,
    RouterIdentity, StaticNexthopType, StaticRoute, TableConfig,
};
use tokio::sync::RwLock;

fn changed_config() -> RoutePlaneConfig {
    RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".into(),
            local_asn: Some(64512),
            router_id_from: None,
        },
        tables: vec![TableConfig {
            name: "master".into(),
            nettype: None,
            kernel_table: None,
            gc_threshold: None,
            gc_period_secs: None,
            sorted: None,
            trie: None,
            min_settle_time_secs: None,
            max_settle_time_secs: None,
        }],
        protocols: vec![ProtocolConfig::Static {
            name: "sched-static".into(),
            table: "master".into(),
            routes: vec![StaticRoute {
                prefix: "203.0.113.0/24".into(),
                next_hop: Some("192.0.2.254".into()),
                blackhole: false,
                address_family: AddressFamily::Ipv4,
                nexthop_type: Some(StaticNexthopType::Router),
                mpls_label: None,
                igp_metric: None,
            }],
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
            mpls_channel: None,
        }],
        ..RoutePlaneConfig::default()
    }
}

fn make_scheduler() -> (Arc<RwLock<ConfigStore>>, Arc<CommitScheduler>) {
    let initial = RoutePlaneConfig::default();
    let store = Arc::new(RwLock::new(ConfigStore::new(initial)));
    let scheduler = Arc::new(CommitScheduler::new(store.clone()));
    (store, scheduler)
}

/// Commit the empty initial config so that the store has a baseline
/// revision (id=1). This is necessary because `undo()` targets the previous
/// revision, and that target must exist.
async fn commit_baseline(store: &Arc<RwLock<ConfigStore>>) {
    let mut s = store.write().await;
    s.commit(CommitRequest {
        author: "test".into(),
        note: "baseline".into(),
    })
    .expect("baseline commit succeeds");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirmed_commit_auto_rolls_back_after_timeout() {
    let (store, scheduler) = make_scheduler();
    commit_baseline(&store).await;

    // Set a candidate config and commit it with a short 1s confirm window.
    {
        let mut s = store.write().await;
        s.replace_candidate(changed_config())
            .expect("candidate is valid");
    }

    let revision = scheduler
        .commit_with_timeout(
            CommitRequest {
                author: "operator".into(),
                note: "risky change".into(),
            },
            1,
        )
        .await
        .expect("commit_with_timeout succeeds");

    // The running config should reflect the new candidate right after commit.
    {
        let s = store.read().await;
        assert_eq!(
            s.running().protocols.len(),
            1,
            "new revision should be running immediately after commit"
        );
    }

    // Wait long enough for the auto-rollback to fire. We give it generous
    // headroom (3s) to absorb CI scheduler jitter.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let s = store.read().await;
    assert!(
        s.running().protocols.is_empty(),
        "auto-rollback should have removed the risky protocol; revisions = {}",
        s.revisions().len()
    );
    assert!(
        s.pending_timeout.is_none(),
        "auto-rollback should clear pending_timeout"
    );
    // Sanity: there should be at least two revisions now (the risky commit
    // plus its rollback).
    assert!(
        s.revisions().len() >= 3,
        "expected at least 3 audit revisions (baseline + risky + rollback), got {}",
        s.revisions().len()
    );
    // The original risky revision id should still be present in the audit log.
    assert!(
        s.revisions().iter().any(|r| r.id == revision.id),
        "original revision should remain in audit log"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirm_within_window_keeps_revision() {
    let (store, scheduler) = make_scheduler();
    commit_baseline(&store).await;

    {
        let mut s = store.write().await;
        s.replace_candidate(changed_config())
            .expect("candidate is valid");
    }

    scheduler
        .commit_with_timeout(
            CommitRequest {
                author: "operator".into(),
                note: "safe change".into(),
            },
            5,
        )
        .await
        .expect("commit_with_timeout succeeds");

    // Confirm after a short delay (well within the 5s window).
    tokio::time::sleep(Duration::from_secs(1)).await;
    scheduler
        .confirm()
        .await
        .expect("confirm within window succeeds");

    // Sleep past the original timeout to make sure the (cancelled) timer
    // does not fire.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let s = store.read().await;
    assert_eq!(
        s.running().protocols.len(),
        1,
        "confirmed commit should still be running after timeout window"
    );
    assert!(
        s.pending_timeout.is_none(),
        "confirm should have cleared pending_timeout"
    );
    assert_eq!(
        s.revisions().len(),
        2,
        "no rollback audit revision should have been recorded (baseline + confirmed)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_pending_aborts_scheduler() {
    let (store, scheduler) = make_scheduler();
    commit_baseline(&store).await;

    {
        let mut s = store.write().await;
        s.replace_candidate(changed_config())
            .expect("candidate is valid");
    }

    scheduler
        .commit_with_timeout(
            CommitRequest {
                author: "operator".into(),
                note: "abort me".into(),
            },
            2,
        )
        .await
        .expect("commit_with_timeout succeeds");

    // Verify the commit is in effect.
    {
        let s = store.read().await;
        assert_eq!(s.running().protocols.len(), 1);
    }

    // Cancel the pending commit (performs an undo and aborts the timer).
    scheduler
        .cancel_pending()
        .await
        .expect("cancel_pending succeeds");

    let s = store.read().await;
    assert!(
        s.running().protocols.is_empty(),
        "cancel_pending should roll back the running config"
    );
    assert!(
        s.pending_timeout.is_none(),
        "pending_timeout should be cleared"
    );

    // Sleep past the original timeout to confirm the (aborted) timer does
    // not fire and re-introduce a rollback revision.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let s = store.read().await;
    // baseline + abort commit + cancel-driven rollback = 3 revisions, no more.
    assert_eq!(
        s.revisions().len(),
        3,
        "no extra rollback should fire after cancel_pending"
    );
}
