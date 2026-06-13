//! Integration tests for supervisor event-bus resilience.
//!
//! These exercise the `netpilotd::supervisor_resilience` module and the
//! pattern used by the daemon's RIB subscriber loop:
//!
//!   1. A child actor panic must NOT bring down the subscriber loop. The
//!      subscriber keeps processing events from other producers on the
//!      same broadcast channel.
//!   2. A lagged subscriber must recover: when a broadcast channel is
//!      flooded past capacity the subscriber gets `RecvError::Lagged`
//!      and must continue receiving subsequent events instead of
//!      silently exiting.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use netpilot_protocol::event::ProtocolEvent;
use netpilotd::supervisor_resilience::{
    recv_error_is_recoverable, spawn_supervised_with_initial_backoff,
};

#[tokio::test(flavor = "current_thread")]
async fn recv_loop_classifies_lagged_as_recoverable() {
    let lagged = tokio::sync::broadcast::error::RecvError::Lagged(7);
    assert!(recv_error_is_recoverable(&lagged));
}

#[tokio::test(flavor = "current_thread")]
async fn recv_loop_classifies_closed_as_terminal() {
    let closed = tokio::sync::broadcast::error::RecvError::Closed;
    assert!(!recv_error_is_recoverable(&closed));
}

/// A child future panicking inside `spawn_supervised` must NOT take down
/// the surrounding subscriber task. We model the subscriber as a
/// sibling task that drains a broadcast channel fed by a separate
/// producer. The producer and subscriber both outlive the panicking
/// child; if `spawn_supervised` had killed the runtime (or hung),
/// `subscriber_done` would never be true.
#[tokio::test(flavor = "current_thread")]
async fn panicking_child_does_not_kill_sibling_subscriber() {
    let (tx, mut rx) = tokio::sync::broadcast::channel::<ProtocolEvent>(16);
    let subscriber_done = Arc::new(AtomicU32::new(0));
    let sd2 = subscriber_done.clone();

    // Subscriber task: drain three events and signal completion.
    let subscriber = tokio::spawn(async move {
        let mut got = 0u32;
        // 1s timeout so we never hang the test if something is wrong.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while got < 3 && tokio::time::Instant::now() < deadline {
            match rx.recv().await {
                Ok(_) => got += 1,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
        sd2.store(got, Ordering::SeqCst);
    });

    // Spawn a supervised future that panics twice and then succeeds.
    // spawn_supervised must survive the panics without aborting siblings.
    // Use a tiny initial backoff (10ms) so the test finishes quickly.
    let counter = Arc::new(AtomicU32::new(0));
    let c2 = counter.clone();
    let _supervised = tokio::spawn(spawn_supervised_with_initial_backoff(
        "panicker".into(),
        Duration::from_millis(10),
        move || {
            let c = c2.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    panic!("intentional panic attempt {}", n);
                }
                // After surviving 2 panics, return cleanly.
            }
        },
    ));

    // Feed the subscriber 3 events spread across the panic window.
    for i in 0..3u32 {
        let _ = tx.send(ProtocolEvent::Stats {
            protocol_name: format!("p{}", i),
            stats: Default::default(),
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Wait for subscriber to finish.
    let _ = tokio::time::timeout(Duration::from_secs(2), subscriber).await;

    assert_eq!(
        subscriber_done.load(Ordering::SeqCst),
        3,
        "subscriber must receive all events even with a sibling panic"
    );
    assert!(
        counter.load(Ordering::SeqCst) >= 1,
        "spawn_supervised must invoke the future at least once"
    );
}

/// A lagged subscriber must continue to receive subsequent events
/// after a `Lagged` notification. We construct a tiny broadcast
/// channel, flood it past capacity, then verify the receiver keeps
/// getting newer events.
#[tokio::test(flavor = "current_thread")]
async fn lagged_subscriber_recovers_and_keeps_receiving() {
    let (tx, mut rx) = tokio::sync::broadcast::channel::<u32>(4);

    // Flood the channel well past its capacity before the subscriber reads.
    for i in 0..100u32 {
        let _ = tx.send(i);
    }
    // Drop the sender so the channel will close once we drain it — otherwise
    // rx.recv() blocks indefinitely once the buffer is empty.
    drop(tx);

    // Drain — first recv may be Lagged (because we sent 100 into a capacity-4
    // channel), then we expect to keep receiving up to the current tail until
    // Closed.
    let mut lagged = false;
    let mut received_after_lag = 0u32;
    let mut last_val: Option<u32> = None;
    loop {
        match rx.recv().await {
            Ok(v) => {
                if lagged {
                    received_after_lag += 1;
                    if let Some(prev) = last_val {
                        assert!(v > prev, "channel must be ordered after recovery");
                    }
                }
                last_val = Some(v);
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_n)) => {
                lagged = true;
                // critical: do NOT exit, continue draining
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }

    assert!(
        lagged,
        "test setup must produce at least one Lagged notification"
    );
    assert!(
        received_after_lag > 0,
        "subscriber must keep receiving events after Lagged (got {} after-lag events)",
        received_after_lag
    );
}

/// End-to-end check that `spawn_supervised` propagates a clean exit
/// without restarting.
#[tokio::test(flavor = "current_thread")]
async fn spawn_supervised_exits_on_clean_return() {
    let counter = Arc::new(AtomicU32::new(0));
    let c2 = counter.clone();
    let task = tokio::spawn(spawn_supervised_with_initial_backoff(
        "clean".into(),
        Duration::from_millis(10),
        move || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        },
    ));
    let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
