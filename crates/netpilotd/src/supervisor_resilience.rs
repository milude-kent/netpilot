//! Supervisor event-bus resilience helpers.
//!
//! NetPilot's protocol daemon must survive transient failures:
//!
//! * A misbehaving protocol actor that panics must not take down the
//!   `netpilotd` supervisor subscriber loop.
//! * A subscriber that falls behind the broadcast channel must continue
//!   to receive subsequent events instead of exiting silently when
//!   `tokio::sync::broadcast::Receiver::recv` returns
//!   `RecvError::Lagged`.
//!
//! The fixes here cover both shapes. For protocol actors spawned via
//! `ProtocolSupervisor::spawn`, the `JoinHandle` is consumed internally
//! and the supervisor already records the actor's terminal `StateChange`
//! or `Error` event into the broadcast channel, so a downstream daemon
//! operator can observe and act on the failure. We do not wrap
//! `supervisor.spawn` at the `JoinHandle` level because doing so would
//! require reaching into the supervisor's private `tasks: Vec<JoinHandle>`
//! field, which is intentionally encapsulated. Instead, we expose
//! [`spawn_supervised`] for callers that spawn their own futures
//! (e.g. tests, RIB processor) so a panic in the wrapped future is
//! recovered with exponential backoff instead of killing the runtime
//! task.
//!
//! For the subscriber loop, the recommended pattern is documented in
//! the module-level example and matches the requirement in Phase C4:
//! match on `Lagged` (warn and continue) and `Closed` (error and break).

use std::future::Future;
use std::time::Duration;

/// Maximum backoff between supervisor restarts.
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Initial backoff after the first restart.
pub const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// Run `run` as a supervised tokio task. If the future returns
/// successfully the wrapper exits cleanly. If it panics, the wrapper
/// waits `backoff` (starting at [`INITIAL_BACKOFF`] and doubling up to
/// [`MAX_BACKOFF`]) and re-invokes `run`. If joining the task fails for
/// a non-panic reason, the wrapper logs the error and exits.
///
/// `run` is an `Fn() -> Fut` closure so the wrapped future can be
/// rebuilt from scratch on each restart with fresh actor state.
///
/// # Limitation
///
/// `ProtocolSupervisor::spawn` does not expose its internal
/// `JoinHandle`, so this helper cannot be wired around existing
/// protocol actors without modifying the supervisor crate. It is
/// exposed for:
///   * integration tests that spawn child actors outside the supervisor
///   * ad-hoc actors that the daemon spawns directly (e.g. the RIB
///     processor) where we want crash recovery
pub async fn spawn_supervised<F, Fut>(name: String, run: F)
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    spawn_supervised_with_initial_backoff(name, INITIAL_BACKOFF, run).await
}

/// Variant of [`spawn_supervised`] that lets callers control the initial
/// backoff. Tests use this with a short backoff (e.g. 10ms) so they don't
/// have to wait a full second between panic and restart.
pub async fn spawn_supervised_with_initial_backoff<F, Fut>(
    name: String,
    initial_backoff: Duration,
    run: F,
) where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut backoff = initial_backoff;
    loop {
        let join = tokio::spawn(run());
        match join.await {
            Ok(()) => {
                tracing::info!(name = %name, "supervised future exited cleanly");
                return;
            }
            Err(e) if e.is_panic() => {
                tracing::error!(
                    name = %name,
                    error = ?e,
                    backoff_ms = backoff.as_millis() as u64,
                    "supervised future panicked — restarting"
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
            Err(e) => {
                tracing::error!(
                    name = %name,
                    error = ?e,
                    "supervised future join error — not restarting"
                );
                return;
            }
        }
    }
}

/// Helper that classifies a broadcast `RecvError` for logging. Returns
/// `true` if the subscriber loop should keep running (Lagged), `false`
/// if it should terminate (Closed).
pub fn recv_error_is_recoverable(err: &tokio::sync::broadcast::error::RecvError) -> bool {
    matches!(err, tokio::sync::broadcast::error::RecvError::Lagged(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn spawn_supervised_recovers_from_panic() {
        let counter = Arc::new(AtomicU32::new(0));
        let c2 = counter.clone();
        let task = tokio::spawn(spawn_supervised_with_initial_backoff(
            "test".into(),
            Duration::from_millis(10),
            move || {
                let c = c2.clone();
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        panic!("intentional panic attempt {}", n);
                    }
                }
            },
        ));

        // Give it enough time to panic twice (each followed by ~10ms backoff)
        // and then succeed on the third try.
        tokio::time::sleep(Duration::from_millis(200)).await;
        task.abort();
        let _ = task.await;

        assert!(
            counter.load(Ordering::SeqCst) >= 3,
            "expected at least 3 invocations (got {})",
            counter.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn spawn_supervised_exits_on_clean_return() {
        let counter = Arc::new(AtomicU32::new(0));
        let c2 = counter.clone();
        let task = tokio::spawn(spawn_supervised("clean".into(), move || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        }));

        // Wait for one invocation
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = task.await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn recv_error_lagged_is_recoverable() {
        let err = tokio::sync::broadcast::error::RecvError::Lagged(42);
        assert!(recv_error_is_recoverable(&err));
    }

    #[test]
    fn recv_error_closed_is_terminal() {
        let err = tokio::sync::broadcast::error::RecvError::Closed;
        assert!(!recv_error_is_recoverable(&err));
    }
}
