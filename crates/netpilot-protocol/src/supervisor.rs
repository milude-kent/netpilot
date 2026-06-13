use crate::actor::ProtocolActor;
use crate::event::ProtocolEvent;
use crate::handle::ProtocolHandle;
use netpilot_config::ProtocolConfig;
use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

const EVENT_CHANNEL_SIZE: usize = 256;

pub struct ProtocolSupervisor {
    handles: HashMap<String, ProtocolHandle>,
    tasks: Vec<JoinHandle<()>>,
    event_tx: broadcast::Sender<ProtocolEvent>,
}

impl Default for ProtocolSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolSupervisor {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_SIZE);
        Self {
            handles: HashMap::new(),
            tasks: Vec::new(),
            event_tx,
        }
    }

    /// Spawn a protocol actor in a tokio task. Returns a handle for daemon→protocol messaging.
    pub fn spawn<A: ProtocolActor>(
        &mut self,
        name: &str,
        config: ProtocolConfig,
        mut actor: A,
    ) -> ProtocolHandle {
        let (tx, rx) = mpsc::channel(64);
        let handle = ProtocolHandle::new(name.to_string(), tx);
        let event_tx = self.event_tx.clone();
        let actor_name = name.to_string();

        // Pass the event sender to the actor before running
        actor.set_event_tx(event_tx.clone());

        let task = tokio::spawn(async move {
            // Send initial state change
            let _ = event_tx.send(ProtocolEvent::StateChange {
                protocol_name: actor_name.clone(),
                new_state: crate::event::ProtocolState::Start,
                message: "protocol started".into(),
            });

            match actor.run(actor_name.clone(), config, rx).await {
                Ok(()) => {
                    let _ = event_tx.send(ProtocolEvent::StateChange {
                        protocol_name: actor_name.clone(),
                        new_state: crate::event::ProtocolState::Down,
                        message: "protocol stopped normally".into(),
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(ProtocolEvent::Error {
                        protocol_name: actor_name.clone(),
                        message: e.to_string(),
                    });
                }
            }
        });

        self.handles.insert(name.to_string(), handle.clone());
        self.tasks.push(task);
        handle
    }

    /// Subscribe to protocol events from all actors.
    pub fn subscribe(&self) -> broadcast::Receiver<ProtocolEvent> {
        self.event_tx.subscribe()
    }

    /// Get a clonable handle to the broadcast sender that actors publish
    /// events on. Used by external subsystems (e.g. gRPC Subscribe in
    /// `Stream` mode) that need to attach their own subscriber without
    /// going through the supervisor's `RwLock`.
    pub fn event_sender(&self) -> broadcast::Sender<ProtocolEvent> {
        self.event_tx.clone()
    }

    /// Get a handle by protocol name.
    pub fn get(&self, name: &str) -> Option<&ProtocolHandle> {
        self.handles.get(name)
    }

    /// List all protocol names.
    pub fn list(&self) -> Vec<&str> {
        self.handles.keys().map(|k| k.as_str()).collect()
    }

    /// Send shutdown to all protocols and wait for tasks to finish.
    pub async fn shutdown_all(self) {
        for handle in self.handles.values() {
            let _ = handle.shutdown().await;
        }
        for task in self.tasks {
            let _ = task.await;
        }
    }
}
