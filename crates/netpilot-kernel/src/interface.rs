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
/// On Linux: uses rtnetlink for actual link/address queries.
/// On macOS: returns hardcoded stub data.
pub struct InterfaceWatcher {
    #[cfg(target_os = "linux")]
    handle: Option<rtnetlink::Handle>,
}

impl InterfaceWatcher {
    #[allow(unreachable_code)]
    pub async fn new() -> Result<Self, KernelError> {
        #[cfg(target_os = "linux")]
        {
            let (connection, handle, _) =
                rtnetlink::new_connection().map_err(|e| KernelError::Netlink(e.to_string()))?;
            // Spawn the netlink connection so it processes messages in the
            // background. The unsolicited-message receiver is dropped; a
            // future implementation can use it to stream InterfaceEvents.
            tokio::spawn(connection);
            return Ok(Self {
                handle: Some(handle),
            });
        }
        Ok(Self {
            #[cfg(target_os = "linux")]
            handle: None,
        })
    }

    /// Stream interface events.
    ///
    /// Currently returns an empty stream. A full implementation would
    /// filter rtnetlink link/address messages and convert them to
    /// InterfaceEvent variants.
    #[allow(unused_mut)]
    pub async fn watch(
        &mut self,
    ) -> Result<impl futures::Stream<Item = InterfaceEvent>, KernelError> {
        Ok(futures::stream::empty())
    }

    /// List all interfaces (snapshot).
    #[allow(unused_mut)]
    pub async fn list(&mut self) -> Result<Vec<InterfaceInfo>, KernelError> {
        #[cfg(target_os = "linux")]
        {
            use futures::TryStreamExt;
            if let Some(ref handle) = self.handle {
                let mut ifaces = Vec::new();
                let mut stream = handle.link().get().execute();
                while let Some(msg) = stream
                    .try_next()
                    .await
                    .map_err(|e| KernelError::Netlink(e.to_string()))?
                {
                    ifaces.push(InterfaceInfo {
                        name: msg
                            .attributes
                            .iter()
                            .find_map(|a| {
                                if let rtnetlink::packet_route::link::LinkAttribute::IfName(
                                    ref name,
                                ) = a
                                {
                                    Some(name.clone())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default(),
                        index: msg.header.index as u32,
                        flags: InterfaceFlags::default(),
                        addresses: Vec::new(),
                        mtu: None,
                    });
                }
                return Ok(ifaces);
            }
        }
        let ifaces = vec![InterfaceInfo {
            name: "lo".into(),
            index: 1,
            flags: InterfaceFlags {
                up: true,
                running: true,
                loopback: true,
                ..Default::default()
            },
            addresses: vec![IfaceAddress {
                prefix: "127.0.0.1/8".into(),
                scope: AddressScope::Host,
            }],
            mtu: Some(65536),
        }];
        Ok(ifaces)
    }
}
