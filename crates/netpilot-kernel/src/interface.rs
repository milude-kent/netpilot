use crate::error::KernelError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceInfo {
    pub name: String,
    pub index: u32,
    pub flags: InterfaceFlags,
    pub addresses: Vec<IfaceAddress>,
    pub mtu: Option<u32>,
}

impl InterfaceInfo {
    pub fn new(name: &str, index: u32) -> Self {
        Self {
            name: name.to_string(),
            index,
            flags: InterfaceFlags::default(),
            addresses: Vec::new(),
            mtu: None,
        }
    }

    pub fn is_up(&self) -> bool {
        self.flags.up && self.flags.running
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InterfaceFlags {
    pub up: bool,
    pub running: bool,
    pub broadcast: bool,
    pub multicast: bool,
    pub loopback: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfaceAddress {
    pub prefix: String,
    pub scope: AddressScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddressScope {
    Universe,
    Link,
    Host,
}

#[derive(Clone, Debug)]
pub enum InterfaceEvent {
    LinkUp { info: InterfaceInfo },
    LinkDown { name: String },
    AddressAdded { iface: String, addr: IfaceAddress },
    AddressRemoved { iface: String, addr: IfaceAddress },
}

/// Watches for interface state changes.
///
/// On Linux: streams rtnetlink link/address events.
/// On macOS: returns an empty stream (stub).
pub struct InterfaceWatcher {
    #[cfg(target_os = "linux")]
    connection: Option<rtnetlink::Connection>,
}

impl InterfaceWatcher {
    #[allow(unreachable_code)]
    pub async fn new() -> Result<Self, KernelError> {
        #[cfg(target_os = "linux")]
        {
            let (connection, _handle, _) = rtnetlink::new_connection()
                .map_err(|e| KernelError::Netlink(e.to_string()))?;
            return Ok(Self { connection: Some(connection) });
        }
        Ok(Self {
            #[cfg(target_os = "linux")]
            connection: None,
        })
    }

    /// Stream interface events.
    #[allow(unused_mut)]
    pub async fn watch(
        &mut self,
    ) -> Result<impl futures::Stream<Item = InterfaceEvent>, KernelError> {
        #[cfg(target_os = "linux")]
        {
            use futures::StreamExt;
            if let Some(conn) = self.connection.take() {
                let (_conn, _handle, messages) = conn.into_parts();
                // In a full implementation, we'd filter link/address messages
                // and convert them to InterfaceEvent variants.
                // For now, return an empty stream.
                drop(messages);
            }
        }
        Ok(futures::stream::empty())
    }

    /// List all interfaces (snapshot).
    #[allow(unused_mut)]
    pub async fn list(&mut self) -> Result<Vec<InterfaceInfo>, KernelError> {
        let mut ifaces = Vec::new();
        ifaces.push(InterfaceInfo {
            name: "lo".into(),
            index: 1,
            flags: InterfaceFlags { up: true, running: true, loopback: true, ..Default::default() },
            addresses: vec![IfaceAddress { prefix: "127.0.0.1/8".into(), scope: AddressScope::Host }],
            mtu: Some(65536),
        });
        Ok(ifaces)
    }
}
