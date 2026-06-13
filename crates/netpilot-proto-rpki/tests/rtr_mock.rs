//! Mock RTR server test. Spawns a tokio TCP listener that loops, accepting
//! connections, parsing a Reset Query, and replying with a Cache Response +
//! three IPv4 Prefix PDUs + End of Data. The RtrClient falls back from v1
//! to v0, so the loop is what lets a single listener serve both the v1
//! probe and the v0 reset_query.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use netpilot_proto_rpki::rtr::encode;
use netpilot_proto_rpki::{RtrClient, RtrRecord};

/// Drive the RTR/0 server side of a single reset_query exchange.
async fn handle_client(mut sock: tokio::net::TcpStream, session_id: u16, serial: u32) {
    let mut hdr = [0u8; 8];
    if sock.read_exact(&mut hdr).await.is_err() {
        return;
    }
    sock.write_all(&encode::cache_response(
        encode::version_v0(),
        session_id,
        serial,
    ))
    .await
    .ok();
    sock.write_all(&encode::ipv4_prefix(
        encode::version_v0(),
        session_id,
        true,
        24,
        24,
        [10, 0, 0, 0],
        0x0001_0001,
    ))
    .await
    .ok();
    sock.write_all(&encode::ipv4_prefix(
        encode::version_v0(),
        session_id,
        true,
        16,
        16,
        [192, 168, 0, 0],
        0x0001_0002,
    ))
    .await
    .ok();
    sock.write_all(&encode::ipv4_prefix(
        encode::version_v0(),
        session_id,
        true,
        23,
        23,
        [172, 16, 0, 0],
        0x0001_0003,
    ))
    .await
    .ok();
    sock.write_all(&encode::end_of_data(
        encode::version_v0(),
        session_id,
        serial,
    ))
    .await
    .ok();
    sock.flush().await.ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn rtr_reset_query_against_mock_cache() {
    // Listener that loops over accept() — serves the v1 probe, the v0
    // re-handshake, and the data path of reset_query.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let session_id = encode::default_session_id();
    let serial = 0x1234_5678u32;
    let serve = Arc::new(Mutex::new(0u32));
    let serve2 = serve.clone();
    let server = tokio::spawn(async move {
        loop {
            let (sock, _) = match listener.accept().await {
                Ok(p) => p,
                Err(_) => break,
            };
            let mut c = serve2.lock().await;
            *c += 1;
            drop(c);
            tokio::spawn(handle_client(sock, session_id, serial));
            // hold the listener open for a beat so the client has time
            // to read everything before the task is dropped.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });

    // Connect — falls back from v1 to v0 and ends up with a live client.
    let mut client = RtrClient::connect(&addr.to_string())
        .await
        .expect("connect to mock RTR cache");
    assert_eq!(client.version(), 0, "should have fallen back to RTR/0");

    // Drive a reset_query and verify the three records.
    let records = client.reset_query().await.expect("reset_query");
    assert_eq!(records.len(), 3, "expected three ROA records");

    let mut asns: Vec<u32> = records
        .iter()
        .map(|r| match r {
            RtrRecord::Ipv4Roa(roa) => roa.asn,
            other => panic!("expected Ipv4Roa, got {:?}", other),
        })
        .collect();
    asns.sort();
    assert_eq!(asns, vec![0x0001_0001, 0x0001_0002, 0x0001_0003]);

    let prefixes: Vec<String> = records
        .iter()
        .map(|r| match r {
            RtrRecord::Ipv4Roa(roa) => roa.prefix.clone(),
            other => panic!("expected Ipv4Roa, got {:?}", other),
        })
        .collect();
    assert!(prefixes.iter().any(|p| p == "10.0.0.0/24"));
    assert!(prefixes.iter().any(|p| p == "192.168.0.0/16"));
    assert!(prefixes.iter().any(|p| p == "172.16.0.0/23"));

    let served = *serve.lock().await;
    assert!(
        served >= 2,
        "expected at least 2 accepts (v1 probe + v0 data), got {}",
        served
    );

    drop(client);
    server.abort();
    let _ = server.await;
}
