//! Minimal RPKI to Router (RTR) client — RFC 6810 + RFC 8210.
//!
//! Implements the RTR protocol exchange over a single TCP connection. Only
//! the cache-initiated session model is supported: the client opens the
//! socket, sends a Reset Query, and ingests the resulting PDU stream until
//! the cache signals End of Data. Incremental updates are obtained via
//! Serial Query, driven by Serial Notify PDUs from the cache.
//!
//! Wire format references:
//!   * RFC 6810 §5 (PDU layout, IPv4/IPv6 Prefix, Router Key, End of Data,
//!     Cache Reset, Cache Response, Error Report, ASPA in §5.9).
//!   * RFC 8210 §5 (Serial Notify, Serial Query, ASPA PDU layout, error
//!     codes 0..=4 + no-data available).
//!
//! Scope intentionally narrow: ROA prefix matching is exact-string (no
//! longest-prefix match), and the Router Key / ASPA (RFC 8210) PDU is
//! parsed but not yet applied to a validation table.

use std::net::ToSocketAddrs;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

pub mod pdu {
    //! PDU type numbers (RFC 6810 §5.1 + RFC 8210 §5).
    pub const SERIAL_NOTIFY: u8 = 0;
    pub const SERIAL_QUERY: u8 = 1;
    pub const RESET_QUERY: u8 = 2;
    pub const CACHE_RESPONSE: u8 = 3;
    pub const IPV4_PREFIX: u8 = 4;
    pub const IPV6_PREFIX: u8 = 6;
    pub const END_OF_DATA: u8 = 7;
    pub const CACHE_RESET: u8 = 8;
    pub const ROUTER_KEY: u8 = 9;
    pub const ERROR_REPORT: u8 = 10;
    pub const ASPA: u8 = 11;
}

pub const RTR_VERSION_0: u8 = 0;
pub const RTR_VERSION_1: u8 = 1;

/// RTR common PDU header length (RFC 6810 §5.1).
const HEADER_LEN: usize = 8;
/// Max PDU size we will read — RFC 6810 §8 caps any PDU at 2^16 - 1 bytes.
const MAX_PDU: usize = 0xFFFF;
/// Default timeout for individual PDU reads.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum RtrError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("address resolution failed for {0}")]
    AddrResolution(String),
    #[error("protocol version mismatch: cache sent {cache}, we support {supported}")]
    ProtocolVersionMismatch { cache: u8, supported: u8 },
    #[error("malformed PDU: {0}")]
    MalformedPdu(String),
    #[error("error report from cache: code={code} msg={msg}")]
    ErrorReport { code: u16, msg: String },
    #[error("cache refused the session (Cache Reset received)")]
    CacheRefusal,
    #[error("no data available (error code 4)")]
    NoDataAvailable,
    #[error("remote closed connection")]
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoaRecord {
    pub prefix: String,
    pub max_len: u8,
    pub asn: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AspaRecord {
    pub customer_as: u32,
    pub providers: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RtrRecord {
    Ipv4Roa(RoaRecord),
    Ipv6Roa(RoaRecord),
    Aspa(AspaRecord),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RtrUpdate {
    Announce(RtrRecord),
    Withdraw(RtrRecord),
}

/// Minimal RTR/0 + RTR/1 client. One instance maps to one session; on
/// Cache Reset or a transport failure the client should be dropped and a
/// fresh `connect` issued (the actor does this via reconnect-with-backoff).
pub struct RtrClient {
    stream: TcpStream,
    session_id: u16,
    serial: u32,
    version: u8,
}

impl RtrClient {
    /// Open a TCP connection to the cache, send a Reset Query, and read the
    /// Cache Response PDU. The session id and serial are then valid and the
    /// caller can issue `reset_query` or `serial_query`.
    pub async fn connect(addr: &str) -> Result<Self, RtrError> {
        let stream = TcpStream::connect(resolve(addr)?).await?;
        stream.set_nodelay(true).ok();

        let mut client = RtrClient {
            stream,
            session_id: 0,
            serial: 0,
            version: RTR_VERSION_0,
        };

        // We don't know the cache's preferred version up front — try v1
        // first (RFC 8210), fall back to v0 on protocol-version-mismatch.
        match client.open_session(RTR_VERSION_1).await {
            Ok(()) => {}
            Err(RtrError::ProtocolVersionMismatch { .. }) => {
                // Reconnect for v0 — Reset Query opens a new session.
                let stream = TcpStream::connect(resolve(addr)?).await?;
                stream.set_nodelay(true).ok();
                client.stream = stream;
                client.version = RTR_VERSION_0;
                client.open_session(RTR_VERSION_0).await?;
            }
            Err(e) => return Err(e),
        }
        Ok(client)
    }

    async fn open_session(&mut self, version: u8) -> Result<(), RtrError> {
        let mut pdu = Vec::with_capacity(HEADER_LEN);
        pdu.push(version);
        pdu.push(pdu::RESET_QUERY);
        pdu.extend_from_slice(&0u16.to_be_bytes()); // zero session id
        pdu.extend_from_slice(&(HEADER_LEN as u32).to_be_bytes());

        tracing::debug!(target: "netpilot.rtr", version, "TX Reset Query");
        self.stream.write_all(&pdu).await?;

        // Read the Cache Response (8 bytes) and capture session_id + serial.
        let header = read_header(&mut self.stream).await?;
        if header.version != version {
            return Err(RtrError::ProtocolVersionMismatch {
                cache: header.version,
                supported: version,
            });
        }
        if header.pdu_type != pdu::CACHE_RESPONSE {
            return Err(RtrError::MalformedPdu(format!(
                "expected Cache Response, got PDU type {}",
                header.pdu_type
            )));
        }
        // Cache Response body = session_id(2) + serial(4) — RFC 6810 §5.3.
        let body = read_exact(&mut self.stream, header.length - HEADER_LEN).await?;
        if body.len() != 6 {
            return Err(RtrError::MalformedPdu(format!(
                "Cache Response body was {} bytes, expected 6",
                body.len()
            )));
        }
        self.session_id = u16::from_be_bytes([body[0], body[1]]);
        self.serial = u32::from_be_bytes([body[2], body[3], body[4], body[5]]);
        self.version = header.version;

        tracing::debug!(
            target: "netpilot.rtr",
            version = self.version,
            session_id = self.session_id,
            serial = self.serial,
            "RX Cache Response"
        );
        Ok(())
    }

    /// Issue a Reset Query and read every PDU up to and including End of
    /// Data. Returns the new full set of ROA + ASPA records.
    pub async fn reset_query(&mut self) -> Result<Vec<RtrRecord>, RtrError> {
        self.send_reset_query().await?;
        let raw = self.read_until_eod(false).await?;
        Ok(raw.into_iter().map(|(_, rec)| rec).collect())
    }

    /// Issue a Serial Query (RFC 8210 §5.2) for incremental updates. The
    /// returned vector carries the Announce/Withdraw flavor.
    pub async fn serial_query(&mut self) -> Result<Vec<RtrUpdate>, RtrError> {
        let mut pdu = Vec::with_capacity(HEADER_LEN + 4);
        pdu.push(self.version);
        pdu.push(pdu::SERIAL_QUERY);
        pdu.extend_from_slice(&self.session_id.to_be_bytes());
        pdu.extend_from_slice(&((HEADER_LEN + 4) as u32).to_be_bytes());
        pdu.extend_from_slice(&self.serial.to_be_bytes());

        tracing::debug!(
            target: "netpilot.rtr",
            session_id = self.session_id,
            serial = self.serial,
            "TX Serial Query"
        );
        self.stream.write_all(&pdu).await?;

        // The serial-query stream also terminates with End of Data, but the
        // PDUs carry an Announce/Withdraw flag rather than absolute data.
        let records = self.read_until_eod(true).await?;
        Ok(records
            .into_iter()
            .map(|(flag, rec)| match flag {
                AddOrWithdraw::Announce => RtrUpdate::Announce(rec),
                AddOrWithdraw::Withdraw => RtrUpdate::Withdraw(rec),
            })
            .collect())
    }

    async fn send_reset_query(&mut self) -> Result<(), RtrError> {
        let mut pdu = Vec::with_capacity(HEADER_LEN);
        pdu.push(self.version);
        pdu.push(pdu::RESET_QUERY);
        pdu.extend_from_slice(&self.session_id.to_be_bytes());
        pdu.extend_from_slice(&(HEADER_LEN as u32).to_be_bytes());

        tracing::debug!(
            target: "netpilot.rtr",
            session_id = self.session_id,
            "TX Reset Query (data)"
        );
        self.stream.write_all(&pdu).await?;
        Ok(())
    }

    /// Read PDUs until End of Data. When `serial_mode` is true the zero/one
    /// flag in IPv4/IPv6 Prefix and ASPA PDUs is interpreted as
    /// Announce/Withdraw and folded into the returned tuple list.
    async fn read_until_eod(
        &mut self,
        serial_mode: bool,
    ) -> Result<Vec<(AddOrWithdraw, RtrRecord)>, RtrError> {
        let mut out = Vec::new();
        loop {
            let header = read_header(&mut self.stream).await?;
            let body_len = header.length.saturating_sub(HEADER_LEN);
            if body_len > MAX_PDU {
                return Err(RtrError::MalformedPdu(format!(
                    "PDU body too large: {} bytes",
                    body_len
                )));
            }
            let body = read_exact(&mut self.stream, body_len).await?;

            match header.pdu_type {
                pdu::END_OF_DATA => {
                    // RFC 6810 §5.7 / RFC 8210 §5.7
                    // body: session_id(2) + serial(4) + refresh(4) + retry(4) + expire(4)
                    if body.len() != 18 {
                        return Err(RtrError::MalformedPdu(format!(
                            "End of Data body was {} bytes, expected 18",
                            body.len()
                        )));
                    }
                    let sid = u16::from_be_bytes([body[0], body[1]]);
                    if sid != self.session_id {
                        return Err(RtrError::MalformedPdu(format!(
                            "End of Data session id {} != ours {}",
                            sid, self.session_id
                        )));
                    }
                    self.serial = u32::from_be_bytes([body[2], body[3], body[4], body[5]]);
                    let refresh = u32::from_be_bytes([body[6], body[7], body[8], body[9]]);
                    let retry = u32::from_be_bytes([body[10], body[11], body[12], body[13]]);
                    let expire = u32::from_be_bytes([body[14], body[15], body[16], body[17]]);
                    tracing::debug!(
                        target: "netpilot.rtr",
                        serial = self.serial,
                        refresh,
                        retry,
                        expire,
                        "RX End of Data"
                    );
                    return Ok(out);
                }
                pdu::IPV4_PREFIX => {
                    let (flag, rec) = parse_ipv4_prefix(&body, header.zero_or_one(body_len))?;
                    // In serial mode we propagate both Announce and Withdraw;
                    // on a Reset we only care about announcements (the cache is
                    // being rebuilt from scratch).
                    if serial_mode || matches!(flag, AddOrWithdraw::Announce) {
                        out.push((flag, rec));
                    }
                }
                pdu::IPV6_PREFIX => {
                    let (flag, rec) = parse_ipv6_prefix(&body, header.zero_or_one(body_len))?;
                    if serial_mode || matches!(flag, AddOrWithdraw::Announce) {
                        out.push((flag, rec));
                    }
                }
                pdu::ASPA => {
                    let (flag, rec) = parse_aspa(&body, header.zero_or_one(body_len))?;
                    if serial_mode || matches!(flag, AddOrWithdraw::Announce) {
                        out.push((flag, rec));
                    }
                }
                pdu::ROUTER_KEY => {
                    tracing::debug!(target: "netpilot.rtr", "RX Router Key (ignored)");
                }
                pdu::CACHE_RESET => {
                    tracing::warn!(target: "netpilot.rtr", "RX Cache Reset");
                    return Err(RtrError::CacheRefusal);
                }
                pdu::ERROR_REPORT => {
                    if body.len() < 8 {
                        return Err(RtrError::MalformedPdu("Error Report body too short".into()));
                    }
                    let code = u16::from_be_bytes([body[0], body[1]]);
                    let msg_len = u32::from_be_bytes([body[2], body[3], body[4], body[5]]) as usize;
                    let msg = if body.len() >= 8 + msg_len {
                        String::from_utf8_lossy(&body[8..8 + msg_len]).into_owned()
                    } else {
                        String::from_utf8_lossy(&body[8..]).into_owned()
                    };
                    tracing::warn!(
                        target: "netpilot.rtr",
                        code,
                        msg = %msg,
                        "RX Error Report"
                    );
                    if code == 4 {
                        return Err(RtrError::NoDataAvailable);
                    }
                    return Err(RtrError::ErrorReport { code, msg });
                }
                pdu::SERIAL_NOTIFY => {
                    tracing::debug!(target: "netpilot.rtr", "RX Serial Notify (will trigger serial_query)");
                }
                other => {
                    tracing::warn!(
                        target: "netpilot.rtr",
                        pdu_type = other,
                        "RX unknown PDU — skipping"
                    );
                }
            }
        }
    }

    pub fn session_id(&self) -> u16 {
        self.session_id
    }
    pub fn serial(&self) -> u32 {
        self.serial
    }
    pub fn version(&self) -> u8 {
        self.version
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddOrWithdraw {
    Announce,
    Withdraw,
}

struct PduHeader {
    version: u8,
    pdu_type: u8,
    length: usize,
}

impl PduHeader {
    /// RFC 8210 §5: bit 0 of the "flags"/zero byte in IPv4/IPv6 Prefix and
    /// ASPA PDUs is the Announce (1) / Withdraw (0) flag. The wire format
    /// carries this in the *header* zero field for some PDUs and the body
    /// zero field for others; both yield the same bit. This helper maps
    /// the total PDU length to the byte the flag lives in for the PDU
    /// types we care about.
    fn zero_or_one(&self, body_len: usize) -> AddOrWithdraw {
        // For IPv4 Prefix (20B), IPv6 Prefix (32B), ASPA (>= 24B), the flag
        // bit lives in the first body byte. We get there by checking the
        // "body_len - 1" position only if a body exists; otherwise default
        // to Announce. The actual reading happens in the parse_* fns that
        // look at body[0].
        let _ = body_len;
        AddOrWithdraw::Announce
    }
}

async fn read_header(stream: &mut TcpStream) -> Result<PduHeader, RtrError> {
    let mut buf = [0u8; HEADER_LEN];
    let n = timeout(IO_TIMEOUT, stream.read_exact(&mut buf))
        .await
        .map_err(|_| {
            RtrError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "header read",
            ))
        })??;
    if n == 0 {
        return Err(RtrError::Closed);
    }
    let version = buf[0];
    let pdu_type = buf[1];
    let length = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    if length < HEADER_LEN {
        return Err(RtrError::MalformedPdu(format!(
            "PDU length {} smaller than header",
            length
        )));
    }
    Ok(PduHeader {
        version,
        pdu_type,
        length,
    })
}

async fn read_exact(stream: &mut TcpStream, n: usize) -> Result<Vec<u8>, RtrError> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let mut buf = vec![0u8; n];
    timeout(IO_TIMEOUT, stream.read_exact(&mut buf))
        .await
        .map_err(|_| {
            RtrError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "body read",
            ))
        })??;
    Ok(buf)
}

fn resolve(addr: &str) -> Result<std::net::SocketAddr, RtrError> {
    addr.to_socket_addrs()
        .map_err(|_| RtrError::AddrResolution(addr.to_string()))?
        .next()
        .ok_or_else(|| RtrError::AddrResolution(addr.to_string()))
}

// --- PDU decoders --------------------------------------------------------

fn parse_ipv4_prefix(
    body: &[u8],
    _flag_hint: AddOrWithdraw,
) -> Result<(AddOrWithdraw, RtrRecord), RtrError> {
    // RFC 6810 §5.4: flags(1) + prefix_len(1) + max_len(1) + zero(1)
    //                + prefix(4) + asn(4) = 12 bytes body, 20 byte total PDU.
    if body.len() != 12 {
        return Err(RtrError::MalformedPdu(format!(
            "IPv4 Prefix body was {} bytes, expected 12",
            body.len()
        )));
    }
    let flag = body[0] & 0x01;
    let prefix_len = body[1];
    let max_len = body[2];
    let prefix_bytes: [u8; 4] = [body[4], body[5], body[6], body[7]];
    let asn = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
    let prefix = format!(
        "{}.{}.{}.{}/{}",
        prefix_bytes[0], prefix_bytes[1], prefix_bytes[2], prefix_bytes[3], prefix_len
    );
    let flag = if flag == 1 {
        AddOrWithdraw::Announce
    } else {
        AddOrWithdraw::Withdraw
    };
    Ok((
        flag,
        RtrRecord::Ipv4Roa(RoaRecord {
            prefix,
            max_len,
            asn,
        }),
    ))
}

fn parse_ipv6_prefix(
    body: &[u8],
    _flag_hint: AddOrWithdraw,
) -> Result<(AddOrWithdraw, RtrRecord), RtrError> {
    // RFC 6810 §5.6: flags(1) + prefix_len(1) + max_len(1) + zero(1)
    //                + prefix(16) + asn(4) = 24 bytes body, 32 byte total PDU.
    if body.len() != 24 {
        return Err(RtrError::MalformedPdu(format!(
            "IPv6 Prefix body was {} bytes, expected 24",
            body.len()
        )));
    }
    let flag = body[0] & 0x01;
    let prefix_len = body[1];
    let max_len = body[2];
    let mut prefix_bytes = [0u8; 16];
    prefix_bytes.copy_from_slice(&body[4..20]);
    let asn = u32::from_be_bytes([body[20], body[21], body[22], body[23]]);
    let prefix = format!(
        "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}/{}",
        u16::from_be_bytes([prefix_bytes[0], prefix_bytes[1]]),
        u16::from_be_bytes([prefix_bytes[2], prefix_bytes[3]]),
        u16::from_be_bytes([prefix_bytes[4], prefix_bytes[5]]),
        u16::from_be_bytes([prefix_bytes[6], prefix_bytes[7]]),
        u16::from_be_bytes([prefix_bytes[8], prefix_bytes[9]]),
        u16::from_be_bytes([prefix_bytes[10], prefix_bytes[11]]),
        u16::from_be_bytes([prefix_bytes[12], prefix_bytes[13]]),
        u16::from_be_bytes([prefix_bytes[14], prefix_bytes[15]]),
        prefix_len
    );
    let flag = if flag == 1 {
        AddOrWithdraw::Announce
    } else {
        AddOrWithdraw::Withdraw
    };
    Ok((
        flag,
        RtrRecord::Ipv6Roa(RoaRecord {
            prefix,
            max_len,
            asn,
        }),
    ))
}

fn parse_aspa(
    body: &[u8],
    _flag_hint: AddOrWithdraw,
) -> Result<(AddOrWithdraw, RtrRecord), RtrError> {
    // RFC 8210 §5.9: flags(1) + zero(1) + provider_count(2 BE) + customer_asn(4) + providers(4*N)
    if body.len() < 8 {
        return Err(RtrError::MalformedPdu(format!(
            "ASPA body too short: {} bytes",
            body.len()
        )));
    }
    let flag = body[0] & 0x01;
    let provider_count = u16::from_be_bytes([body[2], body[3]]) as usize;
    let customer_as = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
    let expected = 8 + provider_count * 4;
    if body.len() != expected {
        return Err(RtrError::MalformedPdu(format!(
            "ASPA body was {} bytes, expected {} for {} providers",
            body.len(),
            expected,
            provider_count
        )));
    }
    let mut providers = Vec::with_capacity(provider_count);
    for i in 0..provider_count {
        let off = 8 + i * 4;
        providers.push(u32::from_be_bytes([
            body[off],
            body[off + 1],
            body[off + 2],
            body[off + 3],
        ]));
    }
    let flag = if flag == 1 {
        AddOrWithdraw::Announce
    } else {
        AddOrWithdraw::Withdraw
    };
    Ok((
        flag,
        RtrRecord::Aspa(AspaRecord {
            customer_as,
            providers,
        }),
    ))
}

// Allow building PDUs from tests without exposing the private `pdu` write
// helpers. These mirror the on-wire layout.
#[doc(hidden)]
pub mod encode {
    use super::{HEADER_LEN, RTR_VERSION_0, RTR_VERSION_1, pdu};

    pub fn cache_response(version: u8, session_id: u16, serial: u32) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(HEADER_LEN + 6);
        pdu.push(version);
        pdu.push(pdu::CACHE_RESPONSE);
        pdu.extend_from_slice(&session_id.to_be_bytes());
        pdu.extend_from_slice(&((HEADER_LEN + 6) as u32).to_be_bytes());
        pdu.extend_from_slice(&session_id.to_be_bytes());
        pdu.extend_from_slice(&serial.to_be_bytes());
        pdu
    }

    pub fn ipv4_prefix(
        version: u8,
        session_id: u16,
        announce: bool,
        prefix_len: u8,
        max_len: u8,
        prefix: [u8; 4],
        asn: u32,
    ) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(20);
        pdu.push(version);
        pdu.push(pdu::IPV4_PREFIX);
        pdu.extend_from_slice(&session_id.to_be_bytes());
        pdu.extend_from_slice(&(20u32).to_be_bytes());
        pdu.push(if announce { 1 } else { 0 });
        pdu.push(prefix_len);
        pdu.push(max_len);
        pdu.push(0);
        pdu.extend_from_slice(&prefix);
        pdu.extend_from_slice(&asn.to_be_bytes());
        pdu
    }

    pub fn end_of_data(version: u8, session_id: u16, serial: u32) -> Vec<u8> {
        let mut pdu = Vec::with_capacity(HEADER_LEN + 18);
        pdu.push(version);
        pdu.push(pdu::END_OF_DATA);
        pdu.extend_from_slice(&session_id.to_be_bytes());
        pdu.extend_from_slice(&((HEADER_LEN + 18) as u32).to_be_bytes());
        pdu.extend_from_slice(&session_id.to_be_bytes());
        pdu.extend_from_slice(&serial.to_be_bytes());
        pdu.extend_from_slice(&3600u32.to_be_bytes()); // refresh
        pdu.extend_from_slice(&300u32.to_be_bytes()); // retry
        pdu.extend_from_slice(&7200u32.to_be_bytes()); // expire
        pdu
    }

    pub fn default_session_id() -> u16 {
        0xCAFE
    }

    pub fn version_v0() -> u8 {
        RTR_VERSION_0
    }
    pub fn version_v1() -> u8 {
        RTR_VERSION_1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ipv4_prefix_basic() {
        let mut body = vec![1u8, 24, 24, 0, 10, 0, 0, 0, 0, 0, 0x1A, 0x19];
        // make the body exactly 12 bytes
        body.resize(12, 0);
        let (flag, rec) = parse_ipv4_prefix(&body, AddOrWithdraw::Announce).unwrap();
        assert_eq!(flag, AddOrWithdraw::Announce);
        match rec {
            RtrRecord::Ipv4Roa(r) => {
                assert_eq!(r.prefix, "10.0.0.0/24");
                assert_eq!(r.max_len, 24);
                assert_eq!(r.asn, 0x1A19);
            }
            _ => panic!("expected Ipv4Roa"),
        }
    }

    #[test]
    fn parse_aspa_basic() {
        // flag=1, zero=0, count=2, customer=0x0001E240, providers=[0x0001E241, 0x0001E242]
        let mut body = vec![1, 0, 0, 2, 0, 0x01, 0xE2, 0x40];
        body.extend_from_slice(&0x0001E241u32.to_be_bytes());
        body.extend_from_slice(&0x0001E242u32.to_be_bytes());
        let (flag, rec) = parse_aspa(&body, AddOrWithdraw::Announce).unwrap();
        assert_eq!(flag, AddOrWithdraw::Announce);
        match rec {
            RtrRecord::Aspa(a) => {
                assert_eq!(a.customer_as, 0x0001E240);
                assert_eq!(a.providers, vec![0x0001E241, 0x0001E242]);
            }
            _ => panic!("expected Aspa"),
        }
    }
}
