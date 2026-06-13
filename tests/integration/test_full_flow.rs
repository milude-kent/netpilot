use std::process::{Command, Child};
use std::time::Duration;
use std::thread;

/// Start netpilotd as a child process, run API tests against it, then kill it.
struct DaemonProcess {
    child: Child,
}

impl DaemonProcess {
    fn start() -> Self {
        let child = Command::new("cargo")
            .args(["run", "-p", "netpilotd"])
            .spawn()
            .expect("failed to start netpilotd");

        // Wait for daemon to be ready
        thread::sleep(Duration::from_secs(2));
        Self { child }
    }

    fn health_check(&self) -> bool {
        reqwest::blocking::get("http://127.0.0.1:8080/health")
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore] // Requires daemon to be running — run with: cargo test --test integration -- --ignored
fn full_config_crud_flow() {
    let daemon = DaemonProcess::start();
    assert!(daemon.health_check(), "daemon should be healthy");

    let client = reqwest::blocking::Client::new();

    // 1. GET running config
    let resp = client.get("http://127.0.0.1:8080/api/config/running")
        .send().expect("GET running config");
    assert!(resp.status().is_success());
    let config: serde_json::Value = resp.json().expect("JSON parse");
    assert_eq!(config["schema_version"], 1);
    assert!(config["tables"].as_array().map_or(false, |t| !t.is_empty()));

    // 2. PUT candidate config
    let candidate = serde_json::json!({
        "schema_version": 1,
        "identity": {"router_id": "192.0.2.1", "local_asn": 64512},
        "protocols": [{
            "kind": "static",
            "name": "test-static",
            "table": "master",
            "routes": [{
                "prefix": "10.0.0.0/8",
                "next_hop": "192.0.2.254",
                "blackhole": false,
                "address_family": "ipv4"
            }]
        }]
    });

    let resp = client.put("http://127.0.0.1:8080/api/config/candidate")
        .json(&candidate)
        .send().expect("PUT candidate");
    assert_eq!(resp.status().as_u16(), 204);

    // 3. GET diff
    let resp = client.get("http://127.0.0.1:8080/api/config/diff")
        .send().expect("GET diff");
    assert!(resp.status().is_success());

    // 4. POST commit
    let commit = serde_json::json!({"author": "integration-test", "note": "test commit"});
    let resp = client.post("http://127.0.0.1:8080/api/config/commit")
        .json(&commit)
        .send().expect("POST commit");
    assert!(resp.status().is_success());

    // 5. Verify running config has committed protocol
    let resp = client.get("http://127.0.0.1:8080/api/config/running")
        .send().expect("GET running");
    let config: serde_json::Value = resp.json().expect("JSON");
    let protocols = config["protocols"].as_array().expect("protocols array");
    assert!(protocols.iter().any(|p| p["name"] == "test-static"));

    // 6. gRPC health check (if gRPC server is running)
    // Would use tonic client — skip for now

    // 7. Web UI serving
    let resp = client.get("http://127.0.0.1:8080/")
        .send().expect("GET web UI");
    assert!(resp.status().is_success());
    let body = resp.text().unwrap();
    assert!(body.contains("NetPilot"));
}

#[test]
#[ignore]
fn rollback_flow() {
    let _daemon = DaemonProcess::start();
    let client = reqwest::blocking::Client::new();

    // Commit initial config
    let initial = serde_json::json!({
        "schema_version": 1,
        "identity": {"router_id": "10.0.0.1"},
        "protocols": [{"kind": "static", "name": "initial", "table": "master", "routes": []}]
    });
    client.put("http://127.0.0.1:8080/api/config/candidate").json(&initial).send().unwrap();
    let resp = client.post("http://127.0.0.1:8080/api/config/commit")
        .json(&serde_json::json!({"author":"test","note":"initial"}))
        .send().unwrap();
    let commit_result: serde_json::Value = resp.json().unwrap();
    let first_revision = commit_result["id"].as_u64().unwrap();

    // Commit second config
    let changed = serde_json::json!({
        "schema_version": 1,
        "identity": {"router_id": "10.0.0.2"},
        "protocols": [{"kind": "static", "name": "changed", "table": "master", "routes": []}]
    });
    client.put("http://127.0.0.1:8080/api/config/candidate").json(&changed).send().unwrap();
    client.post("http://127.0.0.1:8080/api/config/commit")
        .json(&serde_json::json!({"author":"test","note":"changed"}))
        .send().unwrap();

    // Rollback to first revision
    let rollback = serde_json::json!({"revision_id": first_revision, "author": "test", "note": "rollback"});
    let resp = client.post("http://127.0.0.1:8080/api/config/rollback")
        .json(&rollback)
        .send().unwrap();
    assert!(resp.status().is_success());

    // Verify rolled back
    let resp = client.get("http://127.0.0.1:8080/api/config/running").send().unwrap();
    let config: serde_json::Value = resp.json().unwrap();
    assert_eq!(config["identity"]["router_id"], "10.0.0.1");
}

#[test]
#[ignore]
fn gnoi_health_check() {
    // Test that the gRPC server is reachable
    // This test validates that both axum and tonic start successfully
    let _daemon = DaemonProcess::start();

    // Check that REST API works (axum)
    let resp = reqwest::blocking::get("http://127.0.0.1:8080/health").unwrap();
    assert!(resp.status().is_success());

    // Check that gRPC port is open (tonic)
    use std::net::TcpStream;
    let stream = TcpStream::connect("127.0.0.1:50051");
    assert!(stream.is_ok(), "gRPC port 50051 should be listening");
}

#[test]
#[ignore]
fn sse_events_stream() {
    let _daemon = DaemonProcess::start();
    let resp = reqwest::blocking::get("http://127.0.0.1:8080/api/events").unwrap();
    assert!(resp.status().is_success());
    let content_type = resp.headers().get("content-type").map(|v| v.to_str().unwrap_or(""));
    assert!(content_type.map_or(false, |ct| ct.contains("text/event-stream")));
}
