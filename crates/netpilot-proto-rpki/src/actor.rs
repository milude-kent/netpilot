use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats};
use netpilot_protocol::{ProtocolActor, ProtocolError, ProtocolMsg};
use std::collections::HashMap;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, warn};

use crate::rtr::{RtrClient, RtrError, RtrRecord, RtrUpdate};

/// Default RPKI/RTR cache when the config does not name one. Real
/// deployments should override via the Rpki variant of `ProtocolConfig`
/// (open item — schema addition needed in a later phase).
const DEFAULT_CACHE_ADDRESS: &str = "rtr.rpki.cloudflare.com:8282";

pub struct RpkiActor {
    name: String,
    state: ProtocolState,
    roas: HashMap<String, Vec<u32>>, // prefix → allowed ASNs
    aspas: HashMap<u32, Vec<u32>>,   // customer AS → provider AS set
    stats: ProtocolStats,
    cache_address: String,
    have_session: bool,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
}

impl Default for RpkiActor {
    fn default() -> Self {
        Self::new()
    }
}

impl RpkiActor {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            state: ProtocolState::Down,
            roas: HashMap::new(),
            aspas: HashMap::new(),
            stats: ProtocolStats::default(),
            cache_address: DEFAULT_CACHE_ADDRESS.to_string(),
            have_session: false,
            event_tx: None,
        }
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Pull the cache address out of the protocol config. We don't yet
    /// have an Rpki variant in the schema, so we fall back to the default.
    /// The `description` field on most variants is reused as a way to
    /// carry an `rpki://host:port` override for now.
    fn cache_address_from_config(cfg: &ProtocolConfig) -> String {
        let desc = match cfg {
            ProtocolConfig::Static { description, .. }
            | ProtocolConfig::Bgp { description, .. }
            | ProtocolConfig::Ospf { description, .. }
            | ProtocolConfig::Isis { description, .. }
            | ProtocolConfig::Eigrp { description, .. }
            | ProtocolConfig::Pim { description, .. }
            | ProtocolConfig::Rip { description, .. } => description.as_ref(),
            _ => None,
        };
        if let Some(d) = desc
            && let Some(rest) = d.strip_prefix("rpki://")
        {
            return rest.to_string();
        }
        DEFAULT_CACHE_ADDRESS.to_string()
    }

    /// Check ROA validity: does the prefix have the ASN in its allowed list?
    pub fn validate_roa(&self, prefix: &str, asn: u32) -> RoAStatus {
        if let Some(allowed) = self.roas.get(prefix) {
            if allowed.contains(&asn) {
                RoAStatus::Valid
            } else {
                RoAStatus::Invalid
            }
        } else {
            RoAStatus::NotFound
        }
    }

    /// Check ASPA validity for a customer/provider pair.
    pub fn validate_aspa(&self, customer_as: u32, provider_as: u32) -> AspaStatus {
        if let Some(providers) = self.aspas.get(&customer_as) {
            if providers.contains(&provider_as) {
                AspaStatus::Valid
            } else {
                AspaStatus::Invalid
            }
        } else {
            AspaStatus::Unknown
        }
    }

    /// ROA/ASPA count getters for tests + telemetry.
    pub fn roa_count(&self) -> usize {
        self.roas.len()
    }
    pub fn aspa_count(&self) -> usize {
        self.aspas.len()
    }

    /// Test/admin ingest path: apply a single RTR record to the local
    /// tables. Mirrors what a successful Reset/Serial Query would do.
    pub async fn ingest_record(&mut self, record: RtrRecord) {
        self.apply_record(RtrUpdate::Announce(record));
    }

    fn apply_record(&mut self, update: RtrUpdate) {
        match update {
            RtrUpdate::Announce(rec) => self.insert_record(rec),
            RtrUpdate::Withdraw(rec) => self.remove_record(rec),
        }
    }

    fn insert_record(&mut self, rec: RtrRecord) {
        match rec {
            RtrRecord::Ipv4Roa(r) | RtrRecord::Ipv6Roa(r) => {
                self.roas.entry(r.prefix).or_default().push(r.asn);
            }
            RtrRecord::Aspa(a) => {
                self.aspas.insert(a.customer_as, a.providers);
            }
        }
        self.stats.updates_received += 1;
    }

    fn remove_record(&mut self, rec: RtrRecord) {
        match rec {
            RtrRecord::Ipv4Roa(r) | RtrRecord::Ipv6Roa(r) => {
                if let Some(list) = self.roas.get_mut(&r.prefix) {
                    list.retain(|a| *a != r.asn);
                    if list.is_empty() {
                        self.roas.remove(&r.prefix);
                    }
                }
            }
            RtrRecord::Aspa(a) => {
                self.aspas.remove(&a.customer_as);
            }
        }
    }

    /// Drive one refresh against the RTR cache. Reconnect on failure with
    /// an exponential backoff clamped to 60 seconds. The actor must never
    /// crash on cache unavailability.
    async fn refresh_once(&mut self) {
        if !self.have_session {
            self.do_reset_refresh().await
        } else {
            self.do_serial_refresh().await
        }
    }

    async fn do_reset_refresh(&mut self) {
        let addr = self.cache_address.clone();
        debug!(target: "netpilot.proto.rpki", %addr, "connecting to RTR cache");
        let mut backoff = Duration::from_secs(1);
        let mut client = loop {
            match RtrClient::connect(&addr).await {
                Ok(c) => break c,
                Err(e) => {
                    warn!(
                        target: "netpilot.proto.rpki",
                        error = %e,
                        backoff_secs = backoff.as_secs(),
                        "RTR connect failed; will retry"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                }
            }
        };

        match client.reset_query().await {
            Ok(records) => {
                self.roas.clear();
                self.aspas.clear();
                for rec in records {
                    self.insert_record(rec);
                }
                self.have_session = true;
                self.emit_stats();
                debug!(
                    target: "netpilot.proto.rpki",
                    roas = self.roas.len(),
                    aspas = self.aspas.len(),
                    "RTR reset refresh complete"
                );
            }
            Err(e) => {
                warn!(target: "netpilot.proto.rpki", error = %e, "RTR reset_query failed");
                self.have_session = false;
            }
        }
    }

    async fn do_serial_refresh(&mut self) {
        let addr = self.cache_address.clone();
        let mut backoff = Duration::from_secs(1);
        loop {
            match RtrClient::connect(&addr).await {
                Ok(mut client) => match client.serial_query().await {
                    Ok(updates) => {
                        for u in updates {
                            self.apply_record(u);
                        }
                        self.emit_stats();
                        self.have_session = true;
                        return;
                    }
                    Err(RtrError::NoDataAvailable) => {
                        debug!(target: "netpilot.proto.rpki", "RTR serial_query: no data");
                        self.have_session = false;
                        return;
                    }
                    Err(e) => {
                        warn!(
                            target: "netpilot.proto.rpki",
                            error = %e,
                            "RTR serial_query failed"
                        );
                        self.have_session = false;
                        return;
                    }
                },
                Err(e) => {
                    warn!(
                        target: "netpilot.proto.rpki",
                        error = %e,
                        backoff_secs = backoff.as_secs(),
                        "RTR connect for serial refresh failed; backing off"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                }
            }
        }
    }

    fn emit_stats(&self) {
        self.emit(ProtocolEvent::Stats {
            protocol_name: self.name.clone(),
            stats: self.stats.clone(),
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoAStatus {
    Valid,
    Invalid,
    NotFound,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AspaStatus {
    Valid,
    Invalid,
    Unknown,
}

#[async_trait]
impl ProtocolActor for RpkiActor {
    fn set_event_tx(&mut self, tx: tokio::sync::broadcast::Sender<ProtocolEvent>) {
        self.event_tx = Some(tx);
    }

    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name.clone();
        self.cache_address = Self::cache_address_from_config(&config);
        self.state = ProtocolState::Start;
        self.emit(ProtocolEvent::StateChange {
            protocol_name: name.clone(),
            new_state: ProtocolState::Start,
            message: format!("RPKI started; cache={}", self.cache_address),
        });

        let mut refresh_tick = interval(Duration::from_secs(3600));
        refresh_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            select! {
                msg = rx.recv() => {
                    match msg {
                        Some(ProtocolMsg::Shutdown) => {
                            return Err(ProtocolError::Stopped(
                                self.name.clone(),
                                "shutdown".into(),
                            ));
                        }
                        Some(ProtocolMsg::Enable) => {
                            self.state = ProtocolState::Up;
                        }
                        Some(ProtocolMsg::Disable) => {
                            self.state = ProtocolState::Down;
                        }
                        Some(ProtocolMsg::StatusQuery { reply }) => {
                            let _ = reply.send(netpilot_protocol::event::ProtocolStatus {
                                name: self.name.clone(),
                                state: self.state.clone(),
                                uptime_secs: 0,
                                routes_imported: 0,
                                routes_exported: 0,
                            });
                        }
                        None => return Ok(()),
                        _ => {}
                    }
                }
                _ = refresh_tick.tick() => {
                    self.refresh_once().await;
                }
            }
        }
    }
}
