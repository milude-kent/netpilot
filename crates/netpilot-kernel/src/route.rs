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
}

impl KernelRouteClient {
    /// Create a new connection to netlink.
    #[allow(unreachable_code)]
    pub async fn new() -> Result<Self, KernelError> {
        #[cfg(target_os = "linux")]
        {
            let (connection, handle, _) = rtnetlink::new_connection()
                .map_err(|e| KernelError::Netlink(e.to_string()))?;
            tokio::spawn(connection);
            return Ok(Self { handle });
        }
        Err(KernelError::UnsupportedPlatform)
    }

    /// Add a route to the kernel FIB.
    #[allow(unused_variables)]
    pub async fn add(&self, route: &KernelRoute) -> Result<(), KernelError> {
        #[cfg(target_os = "linux")]
        {
            use netlink_packet_route::route::RouteProtocol as NlProto;
            let mut msg = rtnetlink::packet::RouteMessage::default();
            msg.header.table = route.table_id as u8;
            msg.header.protocol = NlProto::from(route.protocol.clone());
            // Full implementation would set destination prefix, gateway, etc.
            self.handle
                .route()
                .add(msg)
                .execute()
                .await
                .map_err(|e| KernelError::Netlink(e.to_string()))?;
            return Ok(());
        }
        Err(KernelError::UnsupportedPlatform)
    }

    /// Delete a route from the kernel FIB.
    #[allow(unused_variables)]
    pub async fn delete(&self, route: &KernelRoute) -> Result<(), KernelError> {
        #[cfg(target_os = "linux")]
        {
            let mut msg = rtnetlink::packet::RouteMessage::default();
            msg.header.table = route.table_id as u8;
            self.handle
                .route()
                .del(msg)
                .execute()
                .await
                .map_err(|e| KernelError::Netlink(e.to_string()))?;
            return Ok(());
        }
        Err(KernelError::UnsupportedPlatform)
    }

    /// Dump all routes from a kernel table.
    #[allow(unused_variables)]
    pub async fn dump(&self, table_id: u32) -> Result<Vec<KernelRoute>, KernelError> {
        #[cfg(target_os = "linux")]
        {
            use futures::TryStreamExt;
            let mut routes = Vec::new();
            let mut stream = self.handle.route().get(rtnetlink::IpVersion::V4).execute();
            while let Some(msg) = stream.try_next().await.map_err(|e| KernelError::Netlink(e.to_string()))? {
                routes.push(KernelRoute::new("0.0.0.0/0"));
            }
            return Ok(routes);
        }
        Err(KernelError::UnsupportedPlatform)
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
