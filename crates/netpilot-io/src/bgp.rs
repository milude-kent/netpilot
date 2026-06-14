use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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
    Multiprotocol {
        afi: u16,
        safi: u8,
    },
    RouteRefresh,
    GracefulRestart {
        flags: u8,
        time_secs: u16,
    },
    FourOctetAsn {
        asn: u32,
    },
    AddPath {
        afi: u16,
        safi: u8,
        send_receive: u8,
    },
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
        let stream = TcpStream::connect(&addr)
            .await
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
                BgpCapability::FourOctetAsn {
                    asn: self.local_asn,
                },
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
            BgpMessage::Open {
                version,
                asn,
                hold_time_secs,
                bgp_identifier,
                capabilities,
            } => {
                let mut body = Vec::new();
                body.push(*version);
                body.extend_from_slice(&asn.to_be_bytes());
                body.extend_from_slice(&hold_time_secs.to_be_bytes());
                // BGP identifier (4 bytes)
                let octets: Vec<u8> = bgp_identifier
                    .split('.')
                    .filter_map(|s| s.parse().ok())
                    .collect();
                body.extend_from_slice(if octets.len() >= 4 {
                    &octets[..4]
                } else {
                    &[0, 0, 0, 0]
                });
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
                        BgpCapability::RouteRefresh => {
                            caps.extend_from_slice(&[2, 0]);
                        }
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
            BgpMessage::Notification {
                error_code,
                error_subcode,
                data,
            } => {
                let mut body = Vec::new();
                body.push(*error_code);
                body.push(*error_subcode);
                body.extend_from_slice(data);
                let total = 19 + body.len();
                buf.extend_from_slice(&(total as u16).to_be_bytes());
                buf.push(3); // NOTIFICATION type
                buf.extend_from_slice(&body);
            }
            _ => {}
        }
        buf
    }

    /// Parse NLRI entries (used for both withdrawn routes and NLRI).
    /// Each entry is: prefix_len (1 byte) + ceil(prefix_len / 8) prefix bytes.
    /// Returns prefixes formatted as "A.B.C.D/prefix_len".
    fn parse_nlri(data: &[u8]) -> Result<Vec<String>, BgpError> {
        let mut prefixes = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            let prefix_len = data[pos] as usize;
            pos += 1;
            let byte_len = prefix_len.div_ceil(8);
            if pos + byte_len > data.len() {
                return Err(BgpError::Protocol("NLRI prefix data truncated".into()));
            }
            let prefix_bytes = &data[pos..pos + byte_len];
            pos += byte_len;

            // Build A.B.C.D string, zero-filling missing octets
            let mut octets = [0u8; 4];
            for (i, &b) in prefix_bytes.iter().enumerate() {
                if i < 4 {
                    octets[i] = b;
                }
            }
            prefixes.push(format!(
                "{}.{}.{}.{}/{}",
                octets[0], octets[1], octets[2], octets[3], prefix_len
            ));
        }
        Ok(prefixes)
    }

    /// Parse path attributes per RFC 4271 §4.3.
    /// Each attribute: flags(1) + type_code(1) + length(1 or 2) + value(variable).
    fn parse_path_attributes(data: &[u8]) -> Result<Vec<BgpAttribute>, BgpError> {
        let mut attrs = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            // Need at least flags + type_code + 1-byte length
            if pos + 3 > data.len() {
                return Err(BgpError::Protocol("path attribute header truncated".into()));
            }
            let flags = data[pos];
            let code = data[pos + 1];
            pos += 2;

            let extended_length = (flags & 0x10) != 0;
            let attr_len = if extended_length {
                if pos + 2 > data.len() {
                    return Err(BgpError::Protocol(
                        "path attribute extended length truncated".into(),
                    ));
                }
                let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                len
            } else {
                let len = data[pos] as usize;
                pos += 1;
                len
            };

            if pos + attr_len > data.len() {
                return Err(BgpError::Protocol("path attribute value truncated".into()));
            }
            let value = data[pos..pos + attr_len].to_vec();
            pos += attr_len;

            attrs.push(BgpAttribute { code, flags, value });
        }
        Ok(attrs)
    }

    /// Decode a BGP message from wire format.
    pub fn decode_message(data: &[u8]) -> Result<BgpMessage, BgpError> {
        if data.len() < 19 {
            return Err(BgpError::Protocol("too short".into()));
        }
        let msg_type = data[18];
        match msg_type {
            1 => Ok(BgpMessage::Open {
                version: data[19],
                asn: u32::from_be_bytes([data[20], data[21], data[22], data[23]]),
                hold_time_secs: u16::from_be_bytes([data[24], data[25]]),
                bgp_identifier: format!("{}.{}.{}.{}", data[26], data[27], data[28], data[29]),
                capabilities: vec![],
            }),
            2 => {
                // UPDATE: RFC 4271 §4.3
                // Body starts at byte 19 (after 16-byte marker + 2-byte length + 1-byte type)
                let body = &data[19..];

                // 1. Withdrawn Routes Length (2 bytes)
                if body.len() < 2 {
                    return Err(BgpError::Protocol(
                        "UPDATE too short for withdrawn routes length".into(),
                    ));
                }
                let withdrawn_len = u16::from_be_bytes([body[0], body[1]]) as usize;
                let offset = 2;

                // 2. Withdrawn Routes
                if body.len() < offset + withdrawn_len {
                    return Err(BgpError::Protocol(
                        "UPDATE withdrawn routes truncated".into(),
                    ));
                }
                let withdrawn_routes = Self::parse_nlri(&body[offset..offset + withdrawn_len])?;
                let offset = offset + withdrawn_len;

                // 3. Total Path Attribute Length (2 bytes)
                if body.len() < offset + 2 {
                    return Err(BgpError::Protocol(
                        "UPDATE too short for path attribute length".into(),
                    ));
                }
                let path_attr_len = u16::from_be_bytes([body[offset], body[offset + 1]]) as usize;
                let offset = offset + 2;

                // 4. Path Attributes
                if body.len() < offset + path_attr_len {
                    return Err(BgpError::Protocol(
                        "UPDATE path attributes truncated".into(),
                    ));
                }
                let path_attributes =
                    Self::parse_path_attributes(&body[offset..offset + path_attr_len])?;
                let offset = offset + path_attr_len;

                // 5. NLRI (remaining bytes)
                let nlri_data = &body[offset..];
                let nlri = Self::parse_nlri(nlri_data)?;

                Ok(BgpMessage::Update {
                    withdrawn_routes,
                    path_attributes,
                    nlri,
                })
            }
            3 => {
                // NOTIFICATION: error_code (1) + error_subcode (1) + data (remaining)
                let body = &data[19..];
                if body.len() < 2 {
                    return Err(BgpError::Protocol(
                        "NOTIFICATION too short for error code/subcode".into(),
                    ));
                }
                let error_code = body[0];
                let error_subcode = body[1];
                let notification_data = body.get(2..).unwrap_or(&[]).to_vec();
                Ok(BgpMessage::Notification {
                    error_code,
                    error_subcode,
                    data: notification_data,
                })
            }
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
        BgpMessage::Update {
            withdrawn_routes: withdrawn,
            path_attributes: attributes,
            nlri,
        }
    }

    /// Encode a full UPDATE message.
    pub fn encode_update(msg: &BgpMessage) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0xFFu8; 16]); // marker

        if let BgpMessage::Update {
            withdrawn_routes,
            path_attributes,
            nlri,
        } = msg
        {
            let mut body = Vec::new();

            // Withdrawn routes
            let mut wr_bytes = Vec::new();
            for prefix in withdrawn_routes {
                let parts: Vec<&str> = prefix.split('/').collect();
                let len: u8 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(32);
                wr_bytes.push(len);
                let byte_len = (len as usize).div_ceil(8);
                let ip_parts: Vec<u8> =
                    parts[0].split('.').filter_map(|s| s.parse().ok()).collect();
                for i in 0..byte_len {
                    wr_bytes.push(ip_parts.get(i).copied().unwrap_or(0));
                }
            }
            body.extend_from_slice(&(wr_bytes.len() as u16).to_be_bytes());
            body.extend_from_slice(&wr_bytes);

            // Path attributes
            let mut attr_bytes = Vec::new();
            for attr in path_attributes {
                attr_bytes.push(attr.flags);
                attr_bytes.push(attr.code);
                if (attr.flags & 0x10) != 0 {
                    // Extended length: 2 bytes
                    attr_bytes.extend_from_slice(&(attr.value.len() as u16).to_be_bytes());
                } else {
                    // Standard length: 1 byte
                    attr_bytes.push(attr.value.len() as u8);
                }
                attr_bytes.extend_from_slice(&attr.value);
            }
            body.extend_from_slice(&(attr_bytes.len() as u16).to_be_bytes());
            body.extend_from_slice(&attr_bytes);

            // NLRI
            let mut nlri_bytes = Vec::new();
            for prefix in nlri {
                let parts: Vec<&str> = prefix.split('/').collect();
                let len: u8 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(32);
                nlri_bytes.push(len);
                let byte_len = (len as usize).div_ceil(8);
                let ip_parts: Vec<u8> =
                    parts[0].split('.').filter_map(|s| s.parse().ok()).collect();
                for i in 0..byte_len {
                    nlri_bytes.push(ip_parts.get(i).copied().unwrap_or(0));
                }
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
    /// Send a KEEPALIVE message to the peer.
    pub async fn send_keepalive(&mut self) -> Result<(), BgpError> {
        let data = Self::encode_message(&BgpMessage::Keepalive);
        if let Some(ref mut stream) = self.stream {
            tokio::io::AsyncWriteExt::write_all(stream, &data)
                .await
                .map_err(|e| BgpError::Io(e.to_string()))?;
            self.messages_sent += 1;
        }
        Ok(())
    }

    pub async fn send_update(&mut self, routes: &[String], next_hop: &str) -> Result<(), BgpError> {
        // Build NEXT_HOP attribute
        let nh_parts: Vec<u8> = next_hop.split('.').filter_map(|s| s.parse().ok()).collect();
        let nh_attr = BgpAttribute {
            flags: 0x40,
            code: 3,
            value: nh_parts,
        };

        // Build ORIGIN attribute (IGP)
        let origin_attr = BgpAttribute {
            flags: 0x40,
            code: 1,
            value: vec![0],
        };

        // Build AS_PATH attribute (empty for now)
        let aspath_attr = BgpAttribute {
            flags: 0x40,
            code: 2,
            value: vec![],
        };

        let update = Self::build_update(
            vec![],
            vec![nh_attr, origin_attr, aspath_attr],
            routes.to_vec(),
        );
        let data = Self::encode_update(&update);
        if let Some(ref mut stream) = self.stream {
            tokio::io::AsyncWriteExt::write_all(stream, &data)
                .await
                .map_err(|e| BgpError::Io(e.to_string()))?;
            self.messages_sent += 1;
        }
        Ok(())
    }

    async fn send_message(&mut self, msg: &BgpMessage) -> Result<(), BgpError> {
        let data = Self::encode_message(msg);
        if let Some(ref mut stream) = self.stream {
            stream
                .write_all(&data)
                .await
                .map_err(|e| BgpError::Io(e.to_string()))?;
            self.messages_sent += 1;
        }
        Ok(())
    }

    pub async fn recv_message(&mut self) -> Result<BgpMessage, BgpError> {
        if let Some(ref mut stream) = self.stream {
            let mut header = vec![0u8; 19];
            stream
                .read_exact(&mut header)
                .await
                .map_err(|e| BgpError::Io(e.to_string()))?;
            let total_len = u16::from_be_bytes([header[16], header[17]]) as usize;
            let mut body = vec![0u8; total_len - 19];
            if !body.is_empty() {
                stream
                    .read_exact(&mut body)
                    .await
                    .map_err(|e| BgpError::Io(e.to_string()))?;
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
            version: 4,
            asn: 65001,
            hold_time_secs: 180,
            bgp_identifier: "192.0.2.1".into(),
            capabilities: vec![],
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
        let attr = BgpAttribute {
            flags: 0x40,
            code: 1,
            value: vec![0],
        };
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

    /// Build a raw UPDATE message from components for testing decode.
    fn build_raw_update(
        withdrawn_routes: &[(&[u8], usize)], // (prefix_bytes, prefix_len) pairs
        path_attrs: &[BgpAttribute],
        nlri: &[(&[u8], usize)],
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        // Marker
        buf.extend_from_slice(&[0xFFu8; 16]);

        let mut body = Vec::new();

        // Withdrawn routes
        let mut wr_bytes = Vec::new();
        for (prefix, plen) in withdrawn_routes {
            wr_bytes.push(*plen as u8);
            wr_bytes.extend_from_slice(prefix);
        }
        body.extend_from_slice(&(wr_bytes.len() as u16).to_be_bytes());
        body.extend_from_slice(&wr_bytes);

        // Path attributes
        let mut pa_bytes = Vec::new();
        for attr in path_attrs {
            pa_bytes.push(attr.flags);
            pa_bytes.push(attr.code);
            if (attr.flags & 0x10) != 0 {
                // Extended length
                pa_bytes.extend_from_slice(&(attr.value.len() as u16).to_be_bytes());
            } else {
                pa_bytes.push(attr.value.len() as u8);
            }
            pa_bytes.extend_from_slice(&attr.value);
        }
        body.extend_from_slice(&(pa_bytes.len() as u16).to_be_bytes());
        body.extend_from_slice(&pa_bytes);

        // NLRI
        let mut nlri_bytes = Vec::new();
        for (prefix, plen) in nlri {
            nlri_bytes.push(*plen as u8);
            nlri_bytes.extend_from_slice(prefix);
        }
        body.extend_from_slice(&nlri_bytes);

        let total = 19 + body.len();
        buf.extend_from_slice(&(total as u16).to_be_bytes());
        buf.push(2); // UPDATE type
        buf.extend_from_slice(&body);
        buf
    }

    #[test]
    fn decode_update_empty() {
        // UPDATE with no withdrawn routes, no path attributes, no NLRI
        let mut msg = Vec::new();
        msg.extend_from_slice(&[0xFFu8; 16]); // marker
        let body_len: u16 = 19 + 4; // header + withdrawn_len(2) + path_attr_len(2)
        msg.extend_from_slice(&body_len.to_be_bytes());
        msg.push(2); // UPDATE type
        msg.extend_from_slice(&0u16.to_be_bytes()); // withdrawn routes length = 0
        msg.extend_from_slice(&0u16.to_be_bytes()); // path attribute length = 0

        let decoded = BgpSession::decode_message(&msg).unwrap();
        match decoded {
            BgpMessage::Update {
                withdrawn_routes,
                path_attributes,
                nlri,
            } => {
                assert!(withdrawn_routes.is_empty());
                assert!(path_attributes.is_empty());
                assert!(nlri.is_empty());
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn decode_update_with_withdrawn_and_nlri() {
        // Withdrawn: 10.0.0.0/8 -> prefix_len=8, prefix_bytes=[10]
        // NLRI: 192.168.0.0/16 -> prefix_len=16, prefix_bytes=[192,168]
        let raw = build_raw_update(&[(&[10u8], 8)], &[], &[(&[192u8, 168], 16)]);

        let decoded = BgpSession::decode_message(&raw).unwrap();
        match decoded {
            BgpMessage::Update {
                withdrawn_routes,
                path_attributes,
                nlri,
            } => {
                assert_eq!(withdrawn_routes, vec!["10.0.0.0/8"]);
                assert!(path_attributes.is_empty());
                assert_eq!(nlri, vec!["192.168.0.0/16"]);
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn decode_update_with_path_attributes() {
        // ORIGIN=IGP, NEXT_HOP=10.0.0.1, AS_PATH empty
        let origin = BgpAttribute {
            flags: 0x40, // transitive
            code: 1,
            value: vec![0], // IGP
        };
        let as_path = BgpAttribute {
            flags: 0x40,
            code: 2,
            value: vec![],
        };
        let next_hop = BgpAttribute {
            flags: 0x40,
            code: 3,
            value: vec![10, 0, 0, 1],
        };

        let raw = build_raw_update(&[], &[origin, as_path, next_hop], &[]);

        let decoded = BgpSession::decode_message(&raw).unwrap();
        match decoded {
            BgpMessage::Update {
                path_attributes, ..
            } => {
                assert_eq!(path_attributes.len(), 3);
                assert_eq!(path_attributes[0].code, 1);
                assert_eq!(path_attributes[0].value, vec![0]);
                assert_eq!(path_attributes[1].code, 2);
                assert_eq!(path_attributes[1].value, Vec::<u8>::new());
                assert_eq!(path_attributes[2].code, 3);
                assert_eq!(path_attributes[2].value, vec![10, 0, 0, 1]);
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn decode_update_extended_length_attribute() {
        // Attribute with extended-length flag (bit 3 = 0x10)
        let mut big_value = vec![0u8; 256];
        big_value[0] = 42;
        let attr = BgpAttribute {
            flags: 0x50, // transitive + extended length
            code: 14,    // MP_REACH_NLRI
            value: big_value.clone(),
        };

        let raw = build_raw_update(&[], &[attr], &[]);
        let decoded = BgpSession::decode_message(&raw).unwrap();
        match decoded {
            BgpMessage::Update {
                path_attributes, ..
            } => {
                assert_eq!(path_attributes.len(), 1);
                assert_eq!(path_attributes[0].code, 14);
                assert_eq!(path_attributes[0].flags, 0x50);
                assert_eq!(path_attributes[0].value.len(), 256);
                assert_eq!(path_attributes[0].value[0], 42);
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn decode_update_multi_octet_nlri() {
        // 10.0.0.0/8 -> [10] prefix_len=8
        // 172.16.0.0/12 -> [172, 16] prefix_len=12 (ceil(12/8)=2 bytes)
        // 192.168.1.0/24 -> [192, 168, 1] prefix_len=24
        let raw = build_raw_update(
            &[],
            &[],
            &[(&[10u8], 8), (&[172u8, 16], 12), (&[192u8, 168, 1], 24)],
        );

        let decoded = BgpSession::decode_message(&raw).unwrap();
        match decoded {
            BgpMessage::Update { nlri, .. } => {
                assert_eq!(nlri.len(), 3);
                assert_eq!(nlri[0], "10.0.0.0/8");
                assert_eq!(nlri[1], "172.16.0.0/12");
                assert_eq!(nlri[2], "192.168.1.0/24");
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn decode_notification() {
        let mut msg = Vec::new();
        msg.extend_from_slice(&[0xFFu8; 16]); // marker
        let total: u16 = 19 + 2 + 3; // header + error_code + error_subcode + 3 data bytes
        msg.extend_from_slice(&total.to_be_bytes());
        msg.push(3); // NOTIFICATION type
        msg.push(2); // error_code: OPEN Message Error
        msg.push(2); // error_subcode: Bad Peer AS
        msg.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // diagnostic data

        let decoded = BgpSession::decode_message(&msg).unwrap();
        match decoded {
            BgpMessage::Notification {
                error_code,
                error_subcode,
                data,
            } => {
                assert_eq!(error_code, 2);
                assert_eq!(error_subcode, 2);
                assert_eq!(data, vec![0xAA, 0xBB, 0xCC]);
            }
            _ => panic!("expected Notification"),
        }
    }

    #[test]
    fn decode_notification_no_data() {
        let mut msg = Vec::new();
        msg.extend_from_slice(&[0xFFu8; 16]);
        let total: u16 = 19 + 2;
        msg.extend_from_slice(&total.to_be_bytes());
        msg.push(3); // NOTIFICATION type
        msg.push(6); // error_code: Cease
        msg.push(3); // error_subcode: Peer Unconfigured

        let decoded = BgpSession::decode_message(&msg).unwrap();
        match decoded {
            BgpMessage::Notification {
                error_code,
                error_subcode,
                data,
            } => {
                assert_eq!(error_code, 6);
                assert_eq!(error_subcode, 3);
                assert!(data.is_empty());
            }
            _ => panic!("expected Notification"),
        }
    }

    #[test]
    fn decode_update_truncated_withdrawn() {
        let mut msg = Vec::new();
        msg.extend_from_slice(&[0xFFu8; 16]);
        let total: u16 = 19 + 4;
        msg.extend_from_slice(&total.to_be_bytes());
        msg.push(2); // UPDATE type
        msg.extend_from_slice(&100u16.to_be_bytes()); // withdrawn routes length = 100, but no data
        msg.extend_from_slice(&0u16.to_be_bytes()); // path attr length

        let result = BgpSession::decode_message(&msg);
        assert!(result.is_err());
        match result.unwrap_err() {
            BgpError::Protocol(msg) => assert!(msg.contains("truncated")),
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn decode_update_truncated_path_attrs() {
        let mut msg = Vec::new();
        msg.extend_from_slice(&[0xFFu8; 16]);
        let total: u16 = 19 + 4;
        msg.extend_from_slice(&total.to_be_bytes());
        msg.push(2); // UPDATE type
        msg.extend_from_slice(&0u16.to_be_bytes()); // withdrawn routes length = 0
        msg.extend_from_slice(&200u16.to_be_bytes()); // path attr length = 200, but no data

        let result = BgpSession::decode_message(&msg);
        assert!(result.is_err());
        match result.unwrap_err() {
            BgpError::Protocol(msg) => assert!(msg.contains("truncated")),
            _ => panic!("expected Protocol error"),
        }
    }

    #[test]
    fn decode_nlri_prefix_len_zero() {
        // 0.0.0.0/0 -> prefix_len=0, no prefix bytes
        let raw = build_raw_update(&[], &[], &[(&[], 0)]);

        let decoded = BgpSession::decode_message(&raw).unwrap();
        match decoded {
            BgpMessage::Update { nlri, .. } => {
                assert_eq!(nlri, vec!["0.0.0.0/0"]);
            }
            _ => panic!("expected Update"),
        }
    }
}
