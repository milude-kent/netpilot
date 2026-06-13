use async_trait::async_trait;
use netpilot_config::ProtocolConfig;
use netpilot_protocol::{ProtocolActor, ProtocolMsg};
use netpilot_protocol::actor::ProtocolError;
use netpilot_protocol::event::{ProtocolEvent, ProtocolState, ProtocolStats, RouteAttributes};
use tokio::sync::mpsc;
use tokio::select;
use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::config::EigrpConfig;
use crate::neighbor::NeighborTable;
use crate::dual::TopologyTable;

pub struct EigrpActor {
    name: String,
    config: EigrpConfig,
    neighbors: NeighborTable,
    topology: TopologyTable,
    event_tx: Option<tokio::sync::broadcast::Sender<ProtocolEvent>>,
    state: ProtocolState,
    stats: ProtocolStats,
    sequence_number: u32,
}

impl EigrpActor {
    pub fn new(config: EigrpConfig) -> Self {
        Self {
            name: config.name.clone(),
            config,
            neighbors: NeighborTable::new(),
            topology: TopologyTable::new(),
            event_tx: None,
            state: ProtocolState::Down,
            stats: ProtocolStats::default(),
            sequence_number: 1,
        }
    }

    pub fn with_event_tx(mut self, tx: tokio::sync::broadcast::Sender<ProtocolEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    fn emit(&self, event: ProtocolEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    fn extract_config(&mut self, config: &ProtocolConfig) {
        if let ProtocolConfig::Eigrp { table, autonomous_system, router_id, interfaces, k_values, maximum_paths, variance, .. } = config {
            self.config.table = table.clone();
            self.config.autonomous_system = *autonomous_system;
            self.config.router_id = router_id.clone();
            self.config.interfaces = interfaces.iter().map(|i| crate::config::EigrpInterfaceConfig {
                interface: i.interface.clone(),
                hello_interval_secs: i.hello_interval_secs,
                hold_time_secs: i.hold_time_secs,
                bandwidth_kbps: i.bandwidth_kbps,
                delay_tens_of_microseconds: i.delay_tens_of_microseconds,
                passive: i.passive,
                split_horizon: i.split_horizon,
            }).collect();
            self.config.k_values = k_values.clone().map(|kv| crate::config::KValues {
                k1: kv.k1,
                k2: kv.k2,
                k3: kv.k3,
                k4: kv.k4,
                k5: kv.k5,
            });
            self.config.maximum_paths = *maximum_paths;
            self.config.variance = *variance;
        }
    }

    async fn handle_msg(&mut self, msg: ProtocolMsg) -> Result<(), ProtocolError> {
        match msg {
            ProtocolMsg::Reload { config, scope: _ } => {
                self.extract_config(&config);
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: self.state.clone(),
                    message: "config reloaded".into(),
                });
                Ok(())
            }
            ProtocolMsg::Enable => {
                self.state = ProtocolState::Up;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Up,
                    message: "enabled".into(),
                });
                Ok(())
            }
            ProtocolMsg::Disable => {
                self.state = ProtocolState::Down;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Down,
                    message: "disabled".into(),
                });
                Ok(())
            }
            ProtocolMsg::Restart => {
                self.state = ProtocolState::Start;
                self.neighbors = NeighborTable::new();
                self.topology = TopologyTable::new();
                self.sequence_number = 1;
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: ProtocolState::Start,
                    message: "restarting".into(),
                });
                Ok(())
            }
            ProtocolMsg::GracefulRestart => {
                self.emit(ProtocolEvent::StateChange {
                    protocol_name: self.name.clone(),
                    new_state: self.state.clone(),
                    message: "graceful restart initiated".into(),
                });
                Ok(())
            }
            ProtocolMsg::Shutdown => {
                self.state = ProtocolState::Down;
                Err(ProtocolError::Stopped(self.name.clone(), "shutdown".into()))
            }
            ProtocolMsg::StatusQuery { reply } => {
                let _ = reply.send(netpilot_protocol::event::ProtocolStatus {
                    name: self.name.clone(),
                    state: self.state.clone(),
                    uptime_secs: 0,
                    routes_imported: self.stats.routes_imported,
                    routes_exported: self.stats.routes_exported,
                });
                Ok(())
            }
        }
    }

    async fn hello_tick(&mut self) {
        // Simulate neighbor processing: each configured interface "sees" a neighbor
        for iface in &self.config.interfaces {
            use crate::neighbor::EigrpNeighbor;
            let neighbor_id = format!("{}-neighbor", iface.interface);
            if let Some(n) = self.neighbors.get_mut(&neighbor_id) {
                n.process_hello();
            } else {
                self.neighbors.upsert(EigrpNeighbor::new(
                    &neighbor_id, &iface.interface, "0.0.0.0",
                    self.config.autonomous_system, 15,
                ));
                if let Some(n) = self.neighbors.get_mut(&neighbor_id) {
                    n.process_hello();
                }
            }
        }
    }

    async fn route_tick(&mut self) {
        // Run DUAL on each topology entry and announce reachable routes
        for entry in self.topology.iter() {
            if let Some(ref successor) = entry.via_neighbor {
                if self.neighbors.get(successor).map_or(false, |n| n.is_up) {
                    self.emit(ProtocolEvent::RouteAnnounce {
                        table: self.config.table.clone(),
                        prefix: entry.prefix.clone(),
                        next_hop: successor.clone(),
                        preference: 90, // EIGRP internal
                        attributes: RouteAttributes {
                            metric: Some(entry.metric.composite()),
                            ..Default::default()
                        },
                    });
                    self.stats.routes_exported += 1;
                }
            }
        }
    }
}

#[async_trait]
impl ProtocolActor for EigrpActor {
    async fn run(
        &mut self,
        name: String,
        config: ProtocolConfig,
        mut rx: mpsc::Receiver<ProtocolMsg>,
    ) -> Result<(), ProtocolError> {
        self.name = name;
        self.state = ProtocolState::Start;
        self.extract_config(&config);

        self.emit(ProtocolEvent::StateChange {
            protocol_name: self.name.clone(),
            new_state: ProtocolState::Start,
            message: "EIGRP protocol started".into(),
        });

        let mut hello_interval = {
            let mut i = interval(Duration::from_secs(5));
            i.set_missed_tick_behavior(MissedTickBehavior::Skip);
            i
        };
        let mut hold_check = {
            let mut i = interval(Duration::from_secs(1));
            i.set_missed_tick_behavior(MissedTickBehavior::Skip);
            i
        };
        let mut route_interval = {
            let mut i = interval(Duration::from_secs(10));
            i.set_missed_tick_behavior(MissedTickBehavior::Skip);
            i
        };

        loop {
            select! {
                msg = rx.recv() => {
                    match msg {
                        Some(m) => {
                            if let Err(e) = self.handle_msg(m).await {
                                self.emit(ProtocolEvent::Error {
                                    protocol_name: self.name.clone(),
                                    message: e.to_string(),
                                });
                                return Err(e);
                            }
                        }
                        None => return Ok(()),
                    }
                }
                _ = hello_interval.tick() => {
                    self.hello_tick().await;
                }
                _ = hold_check.tick() => {
                    let down = self.neighbors.tick_all();
                    for nid in &down {
                        self.emit(ProtocolEvent::StateChange {
                            protocol_name: self.name.clone(),
                            new_state: self.state.clone(),
                            message: format!("neighbor {} down", nid),
                        });
                    }
                }
                _ = route_interval.tick() => {
                    self.route_tick().await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eigrp_actor_creation() {
        let config = EigrpConfig {
            name: "test".into(),
            table: "master".into(),
            autonomous_system: 1,
            router_id: "1.1.1.1".into(),
            interfaces: vec![],
            k_values: None,
            maximum_paths: None,
            variance: None,
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
        };
        let actor = EigrpActor::new(config);
        assert_eq!(actor.state, ProtocolState::Down);
        assert_eq!(actor.sequence_number, 1);
    }
}
