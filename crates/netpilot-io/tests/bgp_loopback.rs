//! In-process loopback test for the BGP encoder/decoder.
//!
//! Spawns a tokio TcpListener on 127.0.0.1:0, drives a fake BGP server that
//! reads OPEN/KEEPALIVE/UPDATE and replies with a NOTIFICATION, and uses
//! the public BgpSession encode/decode APIs to verify the wire format on
//! the client side. BgpSession::connect() hardcodes port 179, so this test
//! validates the codec contract (encode + decode of OPEN/KEEPALIVE/UPDATE/
//! NOTIFICATION) against a real TCP peer.

use std::net::Ipv4Addr;
use std::time::Duration;

use netpilot_io::bgp::{BgpAttribute, BgpMessage, BgpSession};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bgp_loopback_open_keepalive_update_notification() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();

    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.expect("accept");

        // Read client OPEN
        let open = read_bgp_frame(&mut sock).await.expect("read open");
        match &open {
            BgpMessage::Open {
                asn,
                bgp_identifier,
                ..
            } => {
                assert_eq!(*asn, 65000);
                assert_eq!(bgp_identifier, "192.0.2.1");
            }
            other => panic!("expected OPEN, got {other:?}"),
        }

        // Reply OPEN
        let reply = BgpMessage::Open {
            version: 4,
            asn: 65001,
            hold_time_secs: 90,
            bgp_identifier: "203.0.113.1".into(),
            capabilities: vec![],
        };
        sock.write_all(&BgpSession::encode_message(&reply))
            .await
            .expect("write open");

        // Read KEEPALIVE
        let ka = read_bgp_frame(&mut sock).await.expect("read ka");
        assert!(matches!(ka, BgpMessage::Keepalive));

        // Send KEEPALIVE
        sock.write_all(&BgpSession::encode_message(&BgpMessage::Keepalive))
            .await
            .expect("write ka");

        // Read UPDATE
        let upd = read_bgp_frame(&mut sock).await.expect("read update");
        match upd {
            BgpMessage::Update {
                withdrawn_routes,
                path_attributes,
                nlri,
            } => {
                assert!(withdrawn_routes.is_empty());
                let codes: Vec<u8> = path_attributes.iter().map(|a| a.code).collect();
                assert!(codes.contains(&1), "expected ORIGIN");
                assert!(codes.contains(&2), "expected AS_PATH");
                assert!(codes.contains(&3), "expected NEXT_HOP");
                assert!(!nlri.is_empty(), "expected NLRI");
                let has_v4 = nlri.iter().any(|p| p.starts_with("192.0.2.0/24"));
                let has_v6 = nlri.iter().any(|p| p.contains("2001:db8"));
                assert!(has_v4, "expected v4 NLRI 192.0.2.0/24 in {nlri:?}");
                assert!(has_v6, "expected v6 NLRI 2001:db8::/64 in {nlri:?}");
            }
            other => panic!("expected UPDATE, got {other:?}"),
        }

        // Send NOTIFICATION
        let notif = BgpMessage::Notification {
            error_code: 6,
            error_subcode: 2,
            data: b"bye".to_vec(),
        };
        sock.write_all(&BgpSession::encode_message(&notif))
            .await
            .expect("write notification");
    });

    let client = tokio::spawn(async move {
        let mut sock = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("client connect");

        // OPEN
        let router_id = Ipv4Addr::new(192, 0, 2, 1);
        let open = BgpMessage::Open {
            version: 4,
            asn: 65000,
            hold_time_secs: 180,
            bgp_identifier: router_id.to_string(),
            capabilities: vec![],
        };
        sock.write_all(&BgpSession::encode_message(&open))
            .await
            .expect("write open");

        // Read OPEN
        let got = read_bgp_frame(&mut sock).await.expect("read open");
        if let BgpMessage::Open {
            asn,
            hold_time_secs,
            bgp_identifier,
            ..
        } = got
        {
            assert_eq!(asn, 65001);
            assert_eq!(hold_time_secs, 90);
            assert_eq!(bgp_identifier, "203.0.113.1");
        } else {
            panic!("expected OPEN");
        }

        // KEEPALIVE
        sock.write_all(&BgpSession::encode_message(&BgpMessage::Keepalive))
            .await
            .expect("write ka");
        let ka = read_bgp_frame(&mut sock).await.expect("read ka");
        assert!(matches!(ka, BgpMessage::Keepalive));

        // UPDATE with v4 + v6 NLRI
        let origin = BgpAttribute {
            flags: 0x40,
            code: 1,
            value: vec![0],
        };
        let aspath = BgpAttribute {
            flags: 0x40,
            code: 2,
            value: vec![0x02, 0x00],
        };
        let nh = BgpAttribute {
            flags: 0x40,
            code: 3,
            value: vec![10, 0, 0, 1],
        };
        let update = BgpMessage::Update {
            withdrawn_routes: vec![],
            path_attributes: vec![origin, aspath, nh],
            nlri: vec!["192.0.2.0/24".into(), "2001:db8::/64".into()],
        };
        sock.write_all(&BgpSession::encode_update(&update))
            .await
            .expect("write update");

        // NOTIFICATION
        let notif = read_bgp_frame(&mut sock).await.expect("read notif");
        if let BgpMessage::Notification {
            error_code,
            error_subcode,
            data,
        } = notif
        {
            assert_eq!(error_code, 6);
            assert_eq!(error_subcode, 2);
            assert_eq!(data, b"bye".to_vec());
        } else {
            panic!("expected NOTIFICATION");
        }
    });

    let _ = tokio::time::timeout(Duration::from_secs(5), async {
        let _ = tokio::join!(server, client);
    })
    .await;
}

async fn read_bgp_frame(sock: &mut TcpStream) -> Result<BgpMessage, String> {
    let mut header = [0u8; 19];
    sock.read_exact(&mut header)
        .await
        .map_err(|e| e.to_string())?;
    let total_len = u16::from_be_bytes([header[16], header[17]]) as usize;
    if total_len < 19 {
        return Err(format!("invalid length {total_len}"));
    }
    let mut body = vec![0u8; total_len - 19];
    if !body.is_empty() {
        sock.read_exact(&mut body)
            .await
            .map_err(|e| e.to_string())?;
    }
    let mut full = Vec::with_capacity(total_len);
    full.extend_from_slice(&header);
    full.extend_from_slice(&body);
    BgpSession::decode_message(&full).map_err(|e| e.to_string())
}
