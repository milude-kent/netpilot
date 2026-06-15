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
    #[allow(unreachable_code, clippy::needless_return)]
    pub async fn new() -> Result<Self, KernelError> {
        #[cfg(target_os = "linux")]
        {
            let (connection, handle, _) =
                rtnetlink::new_connection().map_err(|e| KernelError::Netlink(e.to_string()))?;
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

    /// Stream interface events using rtnetlink on Linux.
    #[allow(unused_mut, clippy::needless_return)]
    pub async fn watch(
        &mut self,
    ) -> Result<Box<dyn futures::Stream<Item = InterfaceEvent> + Unpin + Send>, KernelError> {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref handle) = self.handle {
                use rtnetlink::packet_route::link::LinkAttribute;

                // Subscribe to link and address changes
                let (link_tx, link_rx) = tokio::sync::mpsc::channel::<InterfaceEvent>(256);

                // Spawn a task that monitors link changes
                let h = handle.clone();
                let link_tx_clone = link_tx.clone();
                tokio::spawn(async move {
                    let mut link_stream = h.link().get().execute();
                    // We need a different approach: listen for netlink events
                    // For now, poll periodically
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                    let mut known: std::collections::HashMap<String, InterfaceInfo> =
                        std::collections::HashMap::new();

                    loop {
                        interval.tick().await;
                        // Poll current interface state
                        let mut new_stream = h.link().get().execute();
                        let mut current: std::collections::HashMap<String, InterfaceInfo> =
                            std::collections::HashMap::new();

                        while let Ok(Some(msg)) = new_stream.try_next().await {
                            let name = msg
                                .attributes
                                .iter()
                                .find_map(|a| {
                                    if let LinkAttribute::IfName(n) = a {
                                        Some(n.clone())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_default();

                            let nl_flags = msg.header.flags;
                            let flags = InterfaceFlags {
                                up: nl_flags.contains(rtnetlink::packet_route::link::LinkFlags::Up),
                                running: nl_flags
                                    .contains(rtnetlink::packet_route::link::LinkFlags::Running),
                                broadcast: nl_flags
                                    .contains(rtnetlink::packet_route::link::LinkFlags::Broadcast),
                                multicast: nl_flags
                                    .contains(rtnetlink::packet_route::link::LinkFlags::Multicast),
                                loopback: nl_flags
                                    .contains(rtnetlink::packet_route::link::LinkFlags::Loopback),
                            };

                            let info = InterfaceInfo {
                                name: name.clone(),
                                index: msg.header.index,
                                flags: flags.clone(),
                                addresses: Vec::new(),
                                mtu: msg.attributes.iter().find_map(|a| {
                                    if let LinkAttribute::Mtu(mtu) = a {
                                        Some(*mtu)
                                    } else {
                                        None
                                    }
                                }),
                            };
                            current.insert(name, info);
                        }

                        // Diff: detect new/up/down interfaces
                        for (name, info) in &current {
                            match known.get(name) {
                                None => {
                                    // New interface
                                    if info.is_up() {
                                        let _ = link_tx_clone
                                            .send(InterfaceEvent::LinkUp { info: info.clone() })
                                            .await;
                                    }
                                }
                                Some(old) => {
                                    if !old.is_up() && info.is_up() {
                                        let _ = link_tx_clone
                                            .send(InterfaceEvent::LinkUp { info: info.clone() })
                                            .await;
                                    } else if old.is_up() && !info.is_up() {
                                        let _ = link_tx_clone
                                            .send(InterfaceEvent::LinkDown { name: name.clone() })
                                            .await;
                                    }
                                }
                            }
                        }

                        // Detect removed interfaces
                        for name in known.keys() {
                            if !current.contains_key(name) {
                                let _ = link_tx_clone
                                    .send(InterfaceEvent::LinkDown { name: name.clone() })
                                    .await;
                            }
                        }

                        known = current;
                    }
                });

                // Convert mpsc receiver to stream
                return Ok(Box::new(futures::stream::unfold(
                    link_rx,
                    |mut rx| async move { rx.recv().await.map(|event| (event, rx)) },
                )));
            }
        }
        Ok(Box::new(futures::stream::empty()))
    }

    /// List all interfaces (snapshot).
    #[allow(unused_mut, clippy::needless_return)]
    pub async fn list(&mut self) -> Result<Vec<InterfaceInfo>, KernelError> {
        #[cfg(target_os = "linux")]
        {
            use futures::TryStreamExt;

            if let Some(ref handle) = self.handle {
                let mut ifaces = Vec::new();

                // Step 1: Collect link information (name, index, flags, mtu)
                let mut links: Vec<(String, u32, InterfaceFlags, Option<u32>)> = Vec::new();
                let mut link_stream = handle.link().get().execute();
                while let Some(msg) = link_stream
                    .try_next()
                    .await
                    .map_err(|e| KernelError::Netlink(e.to_string()))?
                {
                    let name = msg
                        .attributes
                        .iter()
                        .find_map(|a| {
                            if let rtnetlink::packet_route::link::LinkAttribute::IfName(name) = a {
                                Some(name.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    let nl_flags = msg.header.flags;
                    let flags = InterfaceFlags {
                        up: nl_flags.contains(rtnetlink::packet_route::link::LinkFlags::Up),
                        running: nl_flags
                            .contains(rtnetlink::packet_route::link::LinkFlags::Running),
                        broadcast: nl_flags
                            .contains(rtnetlink::packet_route::link::LinkFlags::Broadcast),
                        multicast: nl_flags
                            .contains(rtnetlink::packet_route::link::LinkFlags::Multicast),
                        loopback: nl_flags
                            .contains(rtnetlink::packet_route::link::LinkFlags::Loopback),
                    };

                    let mtu = msg.attributes.iter().find_map(|a| {
                        if let rtnetlink::packet_route::link::LinkAttribute::Mtu(mtu) = a {
                            Some(*mtu)
                        } else {
                            None
                        }
                    });

                    links.push((name, msg.header.index, flags, mtu));
                }

                // Step 2: Collect address information
                let mut addr_map: std::collections::HashMap<u32, Vec<IfaceAddress>> =
                    std::collections::HashMap::new();
                let mut addr_stream = handle.address().get().execute();
                while let Some(msg) = addr_stream
                    .try_next()
                    .await
                    .map_err(|e| KernelError::Netlink(e.to_string()))?
                {
                    let ifindex = msg.header.index;
                    let addr = self.parse_address_message(&msg);
                    if let Some(addr) = addr {
                        addr_map.entry(ifindex).or_default().push(addr);
                    }
                }

                // Step 3: Combine into InterfaceInfo
                for (name, index, flags, mtu) in links {
                    let addresses = addr_map.remove(&index).unwrap_or_default();
                    ifaces.push(InterfaceInfo {
                        name,
                        index,
                        flags,
                        addresses,
                        mtu,
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

    /// Parse an address netlink message into an IfaceAddress.
    #[cfg(target_os = "linux")]
    fn parse_address_message(
        &self,
        msg: &rtnetlink::packet_route::address::AddressMessage,
    ) -> Option<IfaceAddress> {
        use rtnetlink::packet_route::address::AddressAttribute;

        let prefix_len = msg.header.prefix_len;
        let scope = match msg.header.scope {
            rtnetlink::packet_route::address::AddressScope::Universe => AddressScope::Universe,
            rtnetlink::packet_route::address::AddressScope::Site => AddressScope::Universe,
            rtnetlink::packet_route::address::AddressScope::Link => AddressScope::Link,
            rtnetlink::packet_route::address::AddressScope::Host => AddressScope::Host,
            rtnetlink::packet_route::address::AddressScope::Nowhere => AddressScope::Host,
            _ => AddressScope::Universe,
        };

        for attr in &msg.attributes {
            match attr {
                AddressAttribute::Address(ip) => {
                    let prefix = format!("{}/{}", ip, prefix_len);
                    return Some(IfaceAddress { prefix, scope });
                }
                AddressAttribute::Local(ip) => {
                    let prefix = format!("{}/{}", ip, prefix_len);
                    return Some(IfaceAddress { prefix, scope });
                }
                _ => {}
            }
        }
        None
    }
}
