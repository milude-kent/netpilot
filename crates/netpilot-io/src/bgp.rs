use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use time::OffsetDateTime;

/// BGP message types
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BgpMessage {
    Open {
        version: u8,
        asn: u32,
        hold_time_secs: u16,
        bgp_identifier: String,
        capabilities: Vec<BgpCapability>,
    },
    Keepalive,
    Update {
        withdrawn_routes: Vec<String>,
        path_attributes: Vec<BgpAttribute>,
        nlri: Vec<String>,
    },
    Notification {
        error_code: u8,
        error_subcode: u8,
        data: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BgpCapability {
    Multiprotocol { afi: u16, safi: u8 },
    RouteRefresh,
    GracefulRestart { flags: u8, time_secs: u16 },
    FourOctetAsn { asn: u32 },
    AddPath { afi: u16, safi: u8, send_receive: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgpAttribute {
    pub code: u8,
    pub flags: u8,
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BgpSessionState {
    Idle,
    Connect,
    OpenSent,
    OpenConfirm,
    Established,
    Error,
}

/// A BGP TCP session to a single peer.
pub struct BgpSession {
    pub peer_address: String,
    pub local_asn: u32,
    pub remote_asn: u32,
    pub hold_time_secs: u16,
    pub state: BgpSessionState,
    pub established_at: Option<OffsetDateTime>,
    pub messages_received: u64,
    pub messages_sent: u64,
    stream: Option<TcpStream>,
}

impl BgpSession {
    pub fn new(peer_address: &str, local_asn: u32, remote_asn: u32) -> Self {
        Self {
            peer_address: peer_address.to_string(),
            local_asn,
            remote_asn,
            hold_time_secs: 180,
            state: BgpSessionState::Idle,
            established_at: None,
            messages_received: 0,
            messages_sent: 0,
            stream: None,
        }
    }

    /// Connect to the BGP peer (port 179).
    pub async fn connect(&mut self) -> Result<(), BgpError> {
        self.state = BgpSessionState::Connect;
        let addr = format!("{}:179", self.peer_address);
        let stream = TcpStream::connect(&addr).await
            .map_err(|e| BgpError::Connection(e.to_string()))?;
        self.stream = Some(stream);

        // Send OPEN message
        let open = self.build_open();
        self.send_message(&open).await?;
        self.state = BgpSessionState::OpenSent;

        // Receive OPEN message
        let msg = self.recv_message().await?;
        match msg {
            BgpMessage::Open { hold_time_secs, .. } => {
                self.hold_time_secs = hold_time_secs.min(self.hold_time_secs);
                // Send KEEPALIVE
                self.send_message(&BgpMessage::Keepalive).await?;
                self.state = BgpSessionState::OpenConfirm;
                // Receive KEEPALIVE
                let ka = self.recv_message().await?;
                if matches!(ka, BgpMessage::Keepalive) {
                    self.state = BgpSessionState::Established;
                    self.established_at = Some(OffsetDateTime::now_utc());
                }
            }
            _ => {
                self.state = BgpSessionState::Error;
                return Err(BgpError::Protocol("expected OPEN".into()));
            }
        }
        Ok(())
    }

    fn build_open(&self) -> BgpMessage {
        BgpMessage::Open {
            version: 4,
            asn: self.local_asn,
            hold_time_secs: self.hold_time_secs,
            bgp_identifier: "0.0.0.0".to_string(),
            capabilities: vec![
                BgpCapability::Multiprotocol { afi: 1, safi: 1 }, // IPv4 unicast
                BgpCapability::FourOctetAsn { asn: self.local_asn },
                BgpCapability::RouteRefresh,
            ],
        }
    }

    /// Encode a BGP message into wire format.
    pub fn encode_message(msg: &BgpMessage) -> Vec<u8> {
        let mut buf = Vec::new();
        // Marker (16 bytes of 0xFF)
        buf.extend_from_slice(&[0xFFu8; 16]);
        match msg {
            BgpMessage::Open { version, asn, hold_time_secs, bgp_identifier, capabilities } => {
                let mut body = Vec::new();
                body.push(*version);
                body.extend_from_slice(&asn.to_be_bytes());
                body.extend_from_slice(&hold_time_secs.to_be_bytes());
                // BGP identifier (4 bytes)
                let octets: Vec<u8> = bgp_identifier.split('.').filter_map(|s| s.parse().ok()).collect();
                body.extend_from_slice(if octets.len() >= 4 { &octets[..4] } else { &[0,0,0,0] });
                body.push(0); // opt param len
                // Capabilities
                let mut caps = Vec::new();
                for cap in capabilities {
                    match cap {
                        BgpCapability::Multiprotocol { afi, safi } => {
                            caps.extend_from_slice(&[1, 4]);
                            caps.extend_from_slice(&afi.to_be_bytes());
                            caps.push(0);
                            caps.push(*safi);
                        }
                        BgpCapability::RouteRefresh => { caps.extend_from_slice(&[2, 0]); }
                        BgpCapability::FourOctetAsn { asn } => {
                            caps.extend_from_slice(&[65, 4]);
                            caps.extend_from_slice(&asn.to_be_bytes());
                        }
                        _ => {}
                    }
                }
                if !caps.is_empty() {
                    // Set optional parameter flag (byte at index 28)
                    let opt_idx = body.len() - 1;
                    body[opt_idx] = 2;
                    body.push(caps.len() as u8);
                    body.extend_from_slice(&caps);
                }

                let total = 19 + body.len();
                buf.extend_from_slice(&(total as u16).to_be_bytes());
                buf.push(1); // OPEN type
                buf.extend_from_slice(&body);
            }
            BgpMessage::Keepalive => {
                buf.extend_from_slice(&(19u16).to_be_bytes());
                buf.push(4); // KEEPALIVE type
            }
            _ => {}
        }
        buf
    }

    /// Decode a BGP message from wire format.
    pub fn decode_message(data: &[u8]) -> Result<BgpMessage, BgpError> {
        if data.len() < 19 { return Err(BgpError::Protocol("too short".into())); }
        let msg_type = data[18];
        match msg_type {
            1 => Ok(BgpMessage::Open {
                version: data[19],
                asn: u32::from_be_bytes([data[20],data[21],data[22],data[23]]),
                hold_time_secs: u16::from_be_bytes([data[22],data[23]]),
                bgp_identifier: format!("{}.{}.{}.{}", data[24],data[25],data[26],data[27]),
                capabilities: vec![],
            }),
            4 => Ok(BgpMessage::Keepalive),
            _ => Err(BgpError::Protocol(format!("unknown type: {msg_type}"))),
        }
    }

    /// Build a BGP UPDATE message with NLRI and path attributes.
    pub fn build_update(
        withdrawn: Vec<String>,
        attributes: Vec<BgpAttribute>,
        nlri: Vec<String>,
    ) -> BgpMessage {
        BgpMessage::Update { withdrawn_routes: withdrawn, path_attributes: attributes, nlri }
    }

    /// Encode a full UPDATE message.
    pub fn encode_update(msg: &BgpMessage) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0xFFu8; 16]); // marker

        if let BgpMessage::Update { withdrawn_routes: _, path_attributes, nlri } = msg {
            let mut body = Vec::new();

            // Withdrawn routes length + data (simplified: empty for now)
            body.extend_from_slice(&0u16.to_be_bytes());

            // Path attributes
            let mut attr_bytes = Vec::new();
            for attr in path_attributes {
                attr_bytes.push(attr.flags);
                attr_bytes.push(attr.code);
                attr_bytes.extend_from_slice(&(attr.value.len() as u16).to_be_bytes());
                attr_bytes.extend_from_slice(&attr.value);
            }
            body.extend_from_slice(&(attr_bytes.len() as u16).to_be_bytes());
            body.extend_from_slice(&attr_bytes);

            // NLRI (simplified: prefix + length byte)
            let mut nlri_bytes = Vec::new();
            for prefix in nlri {
                let parts: Vec<&str> = prefix.split('/').collect();
                let len: u8 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(32);
                nlri_bytes.push(len);
                // Add prefix bytes (simplified: 4 bytes for IPv4)
                let ip_parts: Vec<u8> = parts[0].split('.').filter_map(|s| s.parse().ok()).collect();
                nlri_bytes.extend_from_slice(&ip_parts);
            }
            body.extend_from_slice(&nlri_bytes);

            let total = 19 + body.len();
            buf.extend_from_slice(&(total as u16).to_be_bytes());
            buf.push(2); // UPDATE type
            buf.extend_from_slice(&body);
        }
        buf
    }

    /// Send routes to a BGP peer.
    pub async fn send_update(&mut self, routes: &[String], next_hop: &str) -> Result<(), BgpError> {
        // Build NEXT_HOP attribute
        let nh_parts: Vec<u8> = next_hop.split('.').filter_map(|s| s.parse().ok()).collect();
        let nh_attr = BgpAttribute { flags: 0x40, code: 3, value: nh_parts };

        // Build ORIGIN attribute (IGP)
        let origin_attr = BgpAttribute { flags: 0x40, code: 1, value: vec![0] };

        // Build AS_PATH attribute (empty for now)
        let aspath_attr = BgpAttribute { flags: 0x40, code: 2, value: vec![] };

        let update = Self::build_update(vec![], vec![nh_attr, origin_attr, aspath_attr], routes.to_vec());
        let data = Self::encode_update(&update);
        if let Some(ref mut stream) = self.stream {
            tokio::io::AsyncWriteExt::write_all(stream, &data).await
                .map_err(|e| BgpError::Io(e.to_string()))?;
            self.messages_sent += 1;
        }
        Ok(())
    }

    async fn send_message(&mut self, msg: &BgpMessage) -> Result<(), BgpError> {
        let data = Self::encode_message(msg);
        if let Some(ref mut stream) = self.stream {
            stream.write_all(&data).await.map_err(|e| BgpError::Io(e.to_string()))?;
            self.messages_sent += 1;
        }
        Ok(())
    }

    async fn recv_message(&mut self) -> Result<BgpMessage, BgpError> {
        if let Some(ref mut stream) = self.stream {
            let mut header = vec![0u8; 19];
            stream.read_exact(&mut header).await.map_err(|e| BgpError::Io(e.to_string()))?;
            let total_len = u16::from_be_bytes([header[16], header[17]]) as usize;
            let mut body = vec![0u8; total_len - 19];
            if !body.is_empty() {
                stream.read_exact(&mut body).await.map_err(|e| BgpError::Io(e.to_string()))?;
            }
            self.messages_received += 1;
            let mut full = header;
            full.extend_from_slice(&body);
            return Self::decode_message(&full);
        }
        Err(BgpError::Connection("not connected".into()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BgpError {
    #[error("connection error: {0}")]
    Connection(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("I/O error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_open_message() {
        let open = BgpMessage::Open {
            version: 4, asn: 65001, hold_time_secs: 180,
            bgp_identifier: "192.0.2.1".into(), capabilities: vec![],
        };
        let encoded = BgpSession::encode_message(&open);
        assert!(encoded.len() >= 29);
        let decoded = BgpSession::decode_message(&encoded).unwrap();
        match decoded {
            BgpMessage::Open { asn, .. } => assert_eq!(asn, 65001),
            _ => panic!("expected Open"),
        }
    }

    #[test]
    fn encode_decode_keepalive() {
        let ka = BgpMessage::Keepalive;
        let encoded = BgpSession::encode_message(&ka);
        assert_eq!(encoded.len(), 19);
        let decoded = BgpSession::decode_message(&encoded).unwrap();
        assert!(matches!(decoded, BgpMessage::Keepalive));
    }

    #[test]
    fn encode_update_with_routes() {
        let attr = BgpAttribute { flags: 0x40, code: 1, value: vec![0] };
        let update = BgpMessage::Update {
            withdrawn_routes: vec![],
            path_attributes: vec![attr],
            nlri: vec!["10.0.0.0/8".into()],
        };
        let encoded = BgpSession::encode_update(&update);
        // Verify UPDATE type byte (type 2 at offset 18)
        assert_eq!(encoded[18], 2);
        // Verify length
        let total = u16::from_be_bytes([encoded[16], encoded[17]]);
        assert!(total as usize >= encoded.len());
    }
}
