//! Prometheus metrics recorder installation.
//!
//! NetPilot uses the `metrics` facade with `metrics-exporter-prometheus`. The
//! exporter itself is *not* run as a separate HTTP listener (C1 auth/policy
//! scope is out of scope here). Instead we install the recorder once at
//! process start, keep the [`PrometheusHandle`], and expose `/metrics` from
//! the same axum router used for the REST API.
//!
//! The recorder install is process-global.  Calling [`install_recorder`]
//! more than once in a process panics.  We use a [`std::sync::OnceLock`] to
//! guarantee at most one install; the first caller pays the cost, every
//! subsequent caller receives a clone of the same handle.

use std::sync::OnceLock;

use metrics::{describe_counter, describe_gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the Prometheus recorder exactly once for this process and return
/// the handle used to render scrape payloads. Subsequent calls return the
/// cached handle without attempting to install a second recorder (which
/// would fail at the global level).
pub fn install_recorder() -> PrometheusHandle {
    HANDLE.get_or_init(install_fresh).clone()
}

fn install_fresh() -> PrometheusHandle {
    let handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("metrics recorder install");

    // Counters
    describe_counter!(
        "netpilot_events_received_total",
        "Total ProtocolEvents received by supervisor subscriber"
    );
    describe_counter!(
        "netpilot_events_lagged_total",
        "Number of times the broadcast channel was lagged"
    );
    describe_counter!(
        "netpilot_fib_routes_installed_total",
        "Routes installed in kernel FIB"
    );
    describe_counter!(
        "netpilot_fib_routes_removed_total",
        "Routes removed from kernel FIB"
    );
    describe_counter!("netpilot_api_requests_total", "HTTP API requests");
    describe_counter!(
        "netpilot_supervisor_restarts_total",
        "Protocol actor restarts triggered by supervisor"
    );
    describe_counter!("netpilot_bgp_messages_sent_total", "BGP messages sent");
    describe_counter!(
        "netpilot_bgp_messages_received_total",
        "BGP messages received"
    );
    describe_counter!("netpilot_rpki_records_total", "RPKI records in cache");

    // Gauges
    describe_gauge!("netpilot_rib_routes_total", "Active routes in RIB");
    describe_gauge!(
        "netpilot_bgp_session_state",
        "BGP session state (0=Idle, 5=Established)"
    );

    handle
}
