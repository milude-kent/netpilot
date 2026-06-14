//! Integration test for `drop_privileges`.
//!
//! This test is gated to Linux and marked `#[ignore]` because it requires
//! root privileges (to drop bounding-set bits) and the presence of a
//! `nobody` user on the host. Run it manually with:
//!
//!     sudo -E cargo test -p netpilotd --test drop_privs -- --ignored --nocapture
//!
//! Implementation: the test uses a self-re-exec pattern. When the test
//! binary is launched with `NETPILOT_DROP_PRIV_TEST_CHILD=1`, it skips
//! the test runner and instead calls `drop_privileges("nobody")`, reads
//! /proc/self/status, prints the resulting CapBnd, and exits 0 on
//! success. The parent test spawns the same binary with that env var set
//! and asserts on the captured stdout.

/// Run by the parent test: spawn the test binary with the child env var
/// set, capture its output, and assert on the CapBnd line.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires root and a `nobody` user"]
fn drop_privs_retains_net_admin_drops_sys_admin() {
    use std::process::Command;

    let exe = std::env::current_exe().expect("current_exe()");

    let child = Command::new(&exe)
        .env("NETPILOT_DROP_PRIV_TEST_CHILD", "1")
        .arg("--exact")
        .arg("drop_privs_retains_net_admin_drops_sys_admin")
        .arg("--ignored")
        .arg("--nocapture")
        .output()
        .expect("spawn child process");

    let stdout = String::from_utf8_lossy(&child.stdout);
    let stderr = String::from_utf8_lossy(&child.stderr);

    assert!(
        child.status.success(),
        "child failed (status: {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        child.status,
        stdout,
        stderr
    );

    // The child prints `CAPBND: <hex>` on its last line of stdout.
    let bnd_line = stdout
        .lines()
        .rev()
        .find(|l| l.starts_with("CAPBND:"))
        .unwrap_or_else(|| {
            panic!(
                "child did not print CAPBND line\n--- stdout ---\n{}\n--- stderr ---\n{}",
                stdout, stderr
            )
        });

    let hex = bnd_line.trim_start_matches("CAPBND:").trim();
    let bnd = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
        .unwrap_or_else(|e| panic!("invalid CapBnd '{}': {}", hex, e));

    assert_eq!(
        (bnd >> 12) & 1,
        1,
        "CapBnd 0x{:x} is missing NET_ADMIN (bit 12)",
        bnd
    );
    assert_eq!(
        (bnd >> 21) & 1,
        0,
        "CapBnd 0x{:x} still contains SYS_ADMIN (bit 21)",
        bnd
    );
    assert_eq!(
        (bnd >> 13) & 1,
        1,
        "CapBnd 0x{:x} is missing NET_RAW (bit 13)",
        bnd
    );
    assert_eq!(
        (bnd >> 10) & 1,
        1,
        "CapBnd 0x{:x} is missing NET_BIND_SERVICE (bit 10)",
        bnd
    );
}

/// Child branch: invoked when `NETPILOT_DROP_PRIV_TEST_CHILD=1` is set in
/// the environment. Calls `drop_privileges("nobody")`, prints the resulting
/// CapBnd on stdout, and exits.
///
/// We detect this at the top of every test in this binary so the test
/// runner does not double-execute the drop logic. The convention used
/// here is: a separate dedicated test function named
/// `drop_privs_child_entry_point` is registered; when run with the env
/// var set, it runs the drop + print path; the parent test invokes this
/// function by exact name with `--ignored --nocapture`.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "child entry point — invoked via self re-exec"]
fn drop_privs_child_entry_point() {
    use std::io::Write as _;

    if std::env::var("NETPILOT_DROP_PRIV_TEST_CHILD").is_err() {
        // Not a child invocation; this test does nothing in the parent.
        return;
    }

    let result = netpilotd::security::drop_privileges("nobody");

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    match result {
        Ok(()) => {
            // Re-read CapBnd after the drop so we can verify on the
            // *child* side too.
            match read_cap_bnd() {
                Ok(bnd) => {
                    let _ = writeln!(handle, "CAPBND: 0x{:x}", bnd);
                    let _ = writeln!(handle, "UID: {}", nix::unistd::getuid().as_raw());
                }
                Err(e) => {
                    let _ = writeln!(handle, "ERROR: {}", e);
                    std::process::exit(2);
                }
            }
        }
        Err(e) => {
            let _ = writeln!(handle, "ERROR: {}", e);
            std::process::exit(3);
        }
    }
}

#[cfg(target_os = "linux")]
fn read_cap_bnd() -> Result<u64, String> {
    let contents = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("read /proc/self/status: {}", e))?;
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("CapBnd:") {
            let hex = rest.trim().trim_start_matches("0x");
            return u64::from_str_radix(hex, 16)
                .map_err(|e| format!("parse CapBnd '{}': {}", hex, e));
        }
    }
    Err("CapBnd not found in /proc/self/status".into())
}

/// Non-Linux stub so the test binary still compiles. Always passes.
#[cfg(not(target_os = "linux"))]
#[test]
fn drop_privs_retains_net_admin_drops_sys_admin() {
    eprintln!("drop_privs integration test is Linux-only; skipping on this platform");
}
