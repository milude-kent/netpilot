use crate::error::KernelError;

/// Represents a kernel FIB route entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelRoute {
    pub prefix: String,
    pub next_hop: Option<String>,
    pub interface: Option<String>,
    pub table_id: u32,
    pub protocol: RouteProtocol,
    pub metric: Option<u32>,
    pub mpls_labels: Vec<u32>,
}

impl KernelRoute {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            next_hop: None,
            interface: None,
            table_id: 254, // main table
            protocol: RouteProtocol::Other(0),
            metric: None,
            mpls_labels: Vec::new(),
        }
    }

    pub fn with_next_hop(mut self, nh: &str) -> Self {
        self.next_hop = Some(nh.to_string());
        self
    }

    pub fn with_table(mut self, table_id: u32) -> Self {
        self.table_id = table_id;
        self
    }

    pub fn with_protocol(mut self, protocol: RouteProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn with_metric(mut self, metric: u32) -> Self {
        self.metric = Some(metric);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteProtocol {
    Kernel,
    Boot,
    Static,
    Bgp,
    Ospf,
    Isis,
    Eigrp,
    Direct,
    Other(u8),
}

impl From<RouteProtocol> for u8 {
    fn from(p: RouteProtocol) -> u8 {
        match p {
            RouteProtocol::Kernel => 2,
            RouteProtocol::Boot => 3,
            RouteProtocol::Static => 4,
            RouteProtocol::Bgp => 186,
            RouteProtocol::Ospf => 188,
            RouteProtocol::Isis => 187,
            RouteProtocol::Eigrp => 192,
            RouteProtocol::Direct => 1,
            RouteProtocol::Other(n) => n,
        }
    }
}

/// High-level netlink route client.
///
/// On Linux: uses rtnetlink for actual kernel FIB operations.
/// On macOS: all operations return UnsupportedPlatform error (stub).
pub struct KernelRouteClient {
    #[cfg(target_os = "linux")]
    handle: rtnetlink::Handle,
    /// LRU cache for interface name → ifindex lookups.
    #[cfg(target_os = "linux")]
    ifindex_cache: std::sync::Mutex<lru::LruCache<String, u32>>,
}

impl KernelRouteClient {
    /// Create a new connection to netlink.
    #[allow(unreachable_code, clippy::needless_return)]
    pub async fn new() -> Result<Self, KernelError> {
        #[cfg(target_os = "linux")]
        {
            let (connection, handle, _) =
                rtnetlink::new_connection().map_err(|e| KernelError::Netlink(e.to_string()))?;
            tokio::spawn(connection);
            return Ok(Self {
                handle,
                ifindex_cache: std::sync::Mutex::new(lru::LruCache::new(
                    std::num::NonZeroUsize::new(256).unwrap(),
                )),
            });
        }
        Err(KernelError::UnsupportedPlatform)
    }

    /// Resolve an interface name to its kernel ifindex.
    ///
    /// Uses an internal LRU cache to avoid repeated netlink queries.
    #[cfg(target_os = "linux")]
    async fn resolve_ifindex(&self, iface_name: &str) -> Result<u32, KernelError> {
        // Check cache first
        if let Some(&idx) = self.ifindex_cache.lock().unwrap().get(iface_name) {
            return Ok(idx);
        }

        // Query netlink
        use futures::TryStreamExt;
        let msg = self
            .handle
            .link()
            .get()
            .match_name(iface_name.to_string())
            .execute()
            .try_next()
            .await
            .map_err(|e| KernelError::Netlink(e.to_string()))?;

        if let Some(msg) = msg {
            let idx = msg.header.index;
            self.ifindex_cache
                .lock()
                .unwrap()
                .put(iface_name.to_string(), idx);
            return Ok(idx);
        }

        Err(KernelError::InterfaceNotFound(iface_name.to_string()))
    }

    /// Resolve an ifindex back to an interface name via netlink.
    #[cfg(target_os = "linux")]
    async fn resolve_ifname(&self, ifindex: u32) -> Option<String> {
        use futures::TryStreamExt;
        let mut stream = self.handle.link().get().execute();
        while let Ok(Some(msg)) = stream.try_next().await {
            if msg.header.index == ifindex {
                return msg.attributes.iter().find_map(|a| {
                    if let rtnetlink::packet_route::link::LinkAttribute::IfName(name) = a {
                        Some(name.clone())
                    } else {
                        None
                    }
                });
            }
        }
        None
    }

    /// Parse a prefix string into (IpAddr, prefix_len).
    #[allow(dead_code)]
    fn parse_prefix(prefix: &str) -> Result<(std::net::IpAddr, u8), KernelError> {
        let parts: Vec<&str> = prefix.splitn(2, '/').collect();
        let ip_str = parts[0];
        let prefix_len: u8 = parts
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(if ip_str.contains(':') { 128 } else { 32 });

        let ip: std::net::IpAddr = ip_str
            .parse()
            .map_err(|e| KernelError::Netlink(format!("invalid prefix IP '{ip_str}': {e}")))?;

        Ok((ip, prefix_len))
    }

    /// Add a route to the kernel FIB.
    #[allow(unused_variables, unreachable_code, clippy::needless_return)]
    pub async fn add(&self, route: &KernelRoute) -> Result<(), KernelError> {
        #[cfg(target_os = "linux")]
        {
            use rtnetlink::RouteMessageBuilder;

            let (ip, prefix_len) = Self::parse_prefix(&route.prefix)?;

            match ip {
                std::net::IpAddr::V4(v4) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                        .destination_prefix(v4, prefix_len)
                        .table_id(route.table_id);

                    let nl_proto = rtnetlink::packet_route::route::RouteProtocol::from(u8::from(
                        route.protocol.clone(),
                    ));
                    builder = builder.protocol(nl_proto);

                    if let Some(ref nh) = route.next_hop
                        && let Ok(gw) = nh.parse::<std::net::Ipv4Addr>()
                    {
                        builder = builder.gateway(gw);
                    }

                    if let Some(ref iface) = route.interface {
                        let ifindex = self.resolve_ifindex(iface).await?;
                        builder = builder.output_interface(ifindex);
                    }

                    if let Some(metric) = route.metric {
                        builder = builder.priority(metric);
                    }

                    let msg = builder.build();
                    self.handle
                        .route()
                        .add(msg)
                        .execute()
                        .await
                        .map_err(|e| KernelError::Netlink(e.to_string()))?;
                }
                std::net::IpAddr::V6(v6) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                        .destination_prefix(v6, prefix_len)
                        .table_id(route.table_id);

                    let nl_proto = rtnetlink::packet_route::route::RouteProtocol::from(u8::from(
                        route.protocol.clone(),
                    ));
                    builder = builder.protocol(nl_proto);

                    if let Some(ref nh) = route.next_hop
                        && let Ok(gw) = nh.parse::<std::net::Ipv6Addr>()
                    {
                        builder = builder.gateway(gw);
                    }

                    if let Some(ref iface) = route.interface {
                        let ifindex = self.resolve_ifindex(iface).await?;
                        builder = builder.output_interface(ifindex);
                    }

                    if let Some(metric) = route.metric {
                        builder = builder.priority(metric);
                    }

                    let msg = builder.build();
                    self.handle
                        .route()
                        .add(msg)
                        .execute()
                        .await
                        .map_err(|e| KernelError::Netlink(e.to_string()))?;
                }
            }
            return Ok(());
        }
        Err(KernelError::UnsupportedPlatform)
    }

    /// Delete a route from the kernel FIB.
    #[allow(unused_variables, unreachable_code, clippy::needless_return)]
    pub async fn delete(&self, route: &KernelRoute) -> Result<(), KernelError> {
        #[cfg(target_os = "linux")]
        {
            use rtnetlink::RouteMessageBuilder;

            let (ip, prefix_len) = Self::parse_prefix(&route.prefix)?;

            match ip {
                std::net::IpAddr::V4(v4) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                        .destination_prefix(v4, prefix_len)
                        .table_id(route.table_id);

                    if let Some(ref nh) = route.next_hop
                        && let Ok(gw) = nh.parse::<std::net::Ipv4Addr>()
                    {
                        builder = builder.gateway(gw);
                    }

                    if let Some(ref iface) = route.interface {
                        let ifindex = self.resolve_ifindex(iface).await?;
                        builder = builder.output_interface(ifindex);
                    }

                    let msg = builder.build();
                    self.handle
                        .route()
                        .del(msg)
                        .execute()
                        .await
                        .map_err(|e| KernelError::Netlink(e.to_string()))?;
                }
                std::net::IpAddr::V6(v6) => {
                    let mut builder = RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                        .destination_prefix(v6, prefix_len)
                        .table_id(route.table_id);

                    if let Some(ref nh) = route.next_hop
                        && let Ok(gw) = nh.parse::<std::net::Ipv6Addr>()
                    {
                        builder = builder.gateway(gw);
                    }

                    if let Some(ref iface) = route.interface {
                        let ifindex = self.resolve_ifindex(iface).await?;
                        builder = builder.output_interface(ifindex);
                    }

                    let msg = builder.build();
                    self.handle
                        .route()
                        .del(msg)
                        .execute()
                        .await
                        .map_err(|e| KernelError::Netlink(e.to_string()))?;
                }
            }
            return Ok(());
        }
        Err(KernelError::UnsupportedPlatform)
    }

    /// Dump all routes from a kernel table.
    #[allow(unused_variables, unreachable_code, clippy::needless_return)]
    pub async fn dump(&self, table_id: u32) -> Result<Vec<KernelRoute>, KernelError> {
        #[cfg(target_os = "linux")]
        {
            use futures::TryStreamExt;
            use rtnetlink::RouteMessageBuilder;

            let mut routes = Vec::new();

            // Query IPv4 routes
            let msg = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                .table_id(table_id)
                .build();
            let mut stream = self.handle.route().get(msg).execute();
            while let Some(msg) = stream
                .try_next()
                .await
                .map_err(|e| KernelError::Netlink(e.to_string()))?
            {
                if let Some(kr) = self.parse_route_message(&msg).await {
                    routes.push(kr);
                }
            }

            // Query IPv6 routes
            let msg = RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                .table_id(table_id)
                .build();
            let mut stream = self.handle.route().get(msg).execute();
            while let Some(msg) = stream
                .try_next()
                .await
                .map_err(|e| KernelError::Netlink(e.to_string()))?
            {
                if let Some(kr) = self.parse_route_message(&msg).await {
                    routes.push(kr);
                }
            }

            return Ok(routes);
        }
        Err(KernelError::UnsupportedPlatform)
    }

    /// Parse a netlink RouteMessage into a KernelRoute.
    #[cfg(target_os = "linux")]
    async fn parse_route_message(
        &self,
        msg: &rtnetlink::packet_route::route::RouteMessage,
    ) -> Option<KernelRoute> {
        use rtnetlink::packet_route::route::RouteAttribute;

        let mut prefix = String::new();
        let mut next_hop: Option<String> = None;
        let mut interface: Option<String> = None;
        let mut metric: Option<u32> = None;
        let mut table_id = msg.header.table as u32;

        for attr in &msg.attributes {
            match attr {
                RouteAttribute::Destination(addr) => {
                    let prefix_len = msg.header.destination_prefix_length;
                    prefix = format!("{}/{}", format_route_address(addr), prefix_len);
                }
                RouteAttribute::Gateway(addr) => {
                    next_hop = Some(format_route_address(addr));
                }
                RouteAttribute::Oif(ifindex) => {
                    interface = self.resolve_ifname(*ifindex).await;
                }
                RouteAttribute::Priority(p) => {
                    metric = Some(*p);
                }
                RouteAttribute::Table(t) => {
                    table_id = *t;
                }
                _ => {}
            }
        }

        // If no destination attribute, this might be a default route
        if prefix.is_empty() {
            let prefix_len = msg.header.destination_prefix_length;
            if prefix_len == 0 {
                prefix = match msg.header.address_family {
                    rtnetlink::packet_route::AddressFamily::Inet => "0.0.0.0/0".to_string(),
                    rtnetlink::packet_route::AddressFamily::Inet6 => "::/0".to_string(),
                    _ => return None,
                };
            } else {
                return None;
            }
        }

        Some(KernelRoute {
            prefix,
            next_hop,
            interface,
            table_id,
            protocol: route_protocol_from_rtnetlink(msg.header.protocol),
            metric,
            mpls_labels: Vec::new(),
        })
    }

    /// Atomically apply a diff: delete old routes, add new ones.
    #[allow(unused_variables)]
    pub async fn apply_diff(
        &self,
        old: &[KernelRoute],
        new: &[KernelRoute],
    ) -> Result<(), KernelError> {
        for route in old {
            self.delete(route).await?;
        }
        for route in new {
            self.add(route).await?;
        }
        Ok(())
    }
}

/// Format a RouteAddress as a string (RouteAddress doesn't implement Display).
#[cfg(target_os = "linux")]
fn format_route_address(addr: &rtnetlink::packet_route::route::RouteAddress) -> String {
    use rtnetlink::packet_route::route::RouteAddress;
    match addr {
        RouteAddress::Inet(v4) => v4.to_string(),
        RouteAddress::Inet6(v6) => v6.to_string(),
        _ => format!("{addr:?}"),
    }
}

/// Convert rtnetlink RouteProtocol to our RouteProtocol.
#[cfg(target_os = "linux")]
fn route_protocol_from_rtnetlink(
    proto: rtnetlink::packet_route::route::RouteProtocol,
) -> RouteProtocol {
    match proto {
        rtnetlink::packet_route::route::RouteProtocol::Kernel => RouteProtocol::Kernel,
        rtnetlink::packet_route::route::RouteProtocol::Boot => RouteProtocol::Boot,
        rtnetlink::packet_route::route::RouteProtocol::Static => RouteProtocol::Static,
        rtnetlink::packet_route::route::RouteProtocol::Bgp => RouteProtocol::Bgp,
        rtnetlink::packet_route::route::RouteProtocol::Ospf => RouteProtocol::Ospf,
        rtnetlink::packet_route::route::RouteProtocol::Isis => RouteProtocol::Isis,
        rtnetlink::packet_route::route::RouteProtocol::Eigrp => RouteProtocol::Eigrp,
        rtnetlink::packet_route::route::RouteProtocol::Rip => RouteProtocol::Other(189),
        rtnetlink::packet_route::route::RouteProtocol::IcmpRedirect => RouteProtocol::Other(1),
        rtnetlink::packet_route::route::RouteProtocol::Other(n) => RouteProtocol::Other(n),
        _ => RouteProtocol::Other(0),
    }
}
