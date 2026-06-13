use crate::route::{NextHop, RouteEntry, RouteKey};

/// Resolves recursive next-hops for a route.
/// A next-hop is "recursive" if it's not directly connected — the RIB
/// must look up a route to the gateway first.
#[derive(Clone, Debug, Default)]
pub struct NextHopResolver;

impl NextHopResolver {
    /// Resolve all next-hops for a route entry. If a next-hop's gateway
    /// has a route in the same table, use that route's next-hop (recursive lookup).
    pub fn resolve(&self, entry: &RouteEntry, table: &crate::table::RouteTable) -> Vec<NextHop> {
        entry
            .next_hops
            .iter()
            .map(|nh| {
                // Check if the gateway is directly reachable via a connected/static route
                let gw_key = RouteKey::prefix(&format!("{}/32", nh.gateway));
                if let Some(gw_route) = table.lookup(&gw_key) {
                    // Recursive: use the gateway's next-hop
                    if let Some(first) = gw_route.next_hops.first() {
                        return NextHop {
                            gateway: first.gateway.clone(),
                            interface: first.interface.clone().or(nh.interface.clone()),
                            weight: nh.weight,
                            mpls_labels: gw_route
                                .mpls_label
                                .iter()
                                .chain(nh.mpls_labels.iter())
                                .copied()
                                .collect(),
                        };
                    }
                }
                nh.clone()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::RouteEntry;
    use crate::table::RouteTable;

    #[test]
    fn resolves_recursive_next_hop() {
        let mut table = RouteTable::new("master");
        let gw = RouteEntry::new(RouteKey::prefix("192.0.2.1/32"), "master", "direct", 0)
            .with_next_hop(NextHop::new("192.0.2.1").with_interface("eth0"));
        table.insert(gw);

        let route = RouteEntry::new(RouteKey::prefix("10.0.0.0/8"), "master", "bgp", 100)
            .with_next_hop(NextHop::new("192.0.2.1"));

        let resolver = NextHopResolver;
        let resolved = resolver.resolve(&route, &table);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].interface, Some("eth0".into()));
    }
}
