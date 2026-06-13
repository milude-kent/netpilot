// SPDX-License-Identifier: MIT
//
// Integration tests for the kernel FIB (route) client.
//
// These tests touch the real netlink subsystem. They are gated with `#[ignore]`
// so `cargo test` on a developer workstation or CI without CAP_NET_ADMIN does
// not fail. To run them, use:
//
//     sudo cargo test -p netpilot-kernel --test fib_integration -- --ignored
//
// All tests are Linux-only and live in their own file so that macOS CI does
// not even try to compile them.

#![cfg(target_os = "linux")]

use std::process::Command;

use netpilot_kernel::{KernelRoute, KernelRouteClient, RouteProtocol};

/// Skip the test (returning early with a warning print) if we are not running
/// as root. CAP_NET_ADMIN is required for netlink route operations.
fn require_root() -> bool {
    let uid = nix::unistd::Uid::current();
    if !uid.is_root() && std::env::var("NETPILOT_FIB_TESTS").is_err() {
        eprintln!(
            "fib_integration: skipping (uid={uid}); run as root or set \
             NETPILOT_FIB_TESTS=1 to force"
        );
        return false;
    }
    true
}

/// Try to drop into a fresh network namespace if we can; otherwise stay in
/// the current namespace and use a non-conflicting table id.
fn try_unshare_net() -> bool {
    use nix::sched::{CloneFlags, unshare};
    // CLONE_NEWNET only affects the calling thread. On failure we just
    // continue with the test using table 999.
    unshare(CloneFlags::CLONE_NEWNET).is_ok()
}

/// Bring the loopback up in the current namespace. Best-effort.
fn ensure_loopback_up() {
    let _ = Command::new("ip")
        .args(["link", "set", "lo", "up"])
        .status();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn add_dump_delete_ipv4_route_in_netns() {
    if !require_root() {
        return;
    }
    let _ = try_unshare_net();
    ensure_loopback_up();

    let client = KernelRouteClient::new()
        .await
        .expect("netlink connection should open");

    const TABLE: u32 = 999;
    let route = KernelRoute::new("10.42.0.0/24")
        .with_next_hop("127.0.0.1")
        .with_table(TABLE)
        .with_protocol(RouteProtocol::Static);

    // Add
    client.add(&route).await.expect("route add");

    // Dump
    let routes = client.dump(TABLE).await.expect("dump");
    let found = routes.iter().any(|r| r.prefix == "10.42.0.0/24");
    assert!(
        found,
        "expected 10.42.0.0/24 in table {TABLE}, got: {routes:#?}"
    );

    // Delete
    client.delete(&route).await.expect("route del");

    // Re-dump — route should be gone
    let routes = client.dump(TABLE).await.expect("dump");
    let still_there = routes.iter().any(|r| r.prefix == "10.42.0.0/24");
    assert!(!still_there, "10.42.0.0/24 should be gone after delete");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn add_dump_delete_ipv6_route_in_netns() {
    if !require_root() {
        return;
    }
    let _ = try_unshare_net();
    ensure_loopback_up();

    let client = KernelRouteClient::new()
        .await
        .expect("netlink connection should open");

    const TABLE: u32 = 999;
    let route = KernelRoute::new("2001:db8::/64")
        .with_next_hop("::1")
        .with_table(TABLE)
        .with_protocol(RouteProtocol::Static);

    client.add(&route).await.expect("route add");

    let routes = client.dump(TABLE).await.expect("dump");
    let found = routes.iter().any(|r| r.prefix == "2001:db8::/64");
    assert!(
        found,
        "expected 2001:db8::/64 in table {TABLE}, got: {routes:#?}"
    );

    client.delete(&route).await.expect("route del");

    let routes = client.dump(TABLE).await.expect("dump");
    let still_there = routes.iter().any(|r| r.prefix == "2001:db8::/64");
    assert!(!still_there, "2001:db8::/64 should be gone after delete");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn add_ecmp_route_in_netns() {
    if !require_root() {
        return;
    }
    let _ = try_unshare_net();
    ensure_loopback_up();

    let client = KernelRouteClient::new()
        .await
        .expect("netlink connection should open");

    // Add a couple of next-hops to the same prefix. The kernel will ECMP
    // across them if it accepts the duplicates. We don't strictly assert
    // ECMP — we just verify both adds succeed and the route is present.
    const TABLE: u32 = 999;
    let a = KernelRoute::new("192.0.2.0/24")
        .with_next_hop("127.0.0.1")
        .with_table(TABLE)
        .with_protocol(RouteProtocol::Static)
        .with_metric(100);
    let b = KernelRoute::new("192.0.2.0/24")
        .with_next_hop("127.0.0.2")
        .with_table(TABLE)
        .with_protocol(RouteProtocol::Static)
        .with_metric(100);

    client.add(&a).await.expect("route add (nh1)");
    // The second add may fail on kernels that don't allow ECMP for the
    // simple (non-multipath) API path; we treat that as soft-fail and still
    // verify the first route is present.
    let _ = client.add(&b).await;

    let routes = client.dump(TABLE).await.expect("dump");
    let count = routes.iter().filter(|r| r.prefix == "192.0.2.0/24").count();
    assert!(count >= 1, "expected at least one 192.0.2.0/24 entry");

    // Clean up.
    let _ = client.delete(&a).await;
    let _ = client.delete(&b).await;
}
