use routeplane_config::{
    AddressFamily, ProtocolConfig, RoutePlaneConfig, RouterIdentity, StaticRoute, TableConfig,
};

#[test]
fn default_config_has_main_table_and_schema_version() {
    let config = RoutePlaneConfig::default();

    assert_eq!(config.schema_version, 1);
    assert_eq!(config.tables.len(), 1);
    assert_eq!(config.tables[0].name, "master");
}

#[test]
fn static_route_config_round_trips_as_json() {
    let config = RoutePlaneConfig {
        identity: RouterIdentity {
            router_id: "192.0.2.1".to_string(),
            local_asn: Some(64512),
        },
        tables: vec![TableConfig {
            name: "edge".to_string(),
            kernel_table: Some(100),
        }],
        protocols: vec![ProtocolConfig::Static {
            name: "static-edge".to_string(),
            table: "edge".to_string(),
            routes: vec![StaticRoute {
                prefix: "203.0.113.0/24".to_string(),
                next_hop: Some("192.0.2.254".to_string()),
                blackhole: false,
                address_family: AddressFamily::Ipv4,
            }],
        }],
        ..RoutePlaneConfig::default()
    };

    let encoded = serde_json::to_string(&config).expect("config serializes");
    let decoded: RoutePlaneConfig = serde_json::from_str(&encoded).expect("config deserializes");

    assert_eq!(decoded.identity.router_id, "192.0.2.1");
    assert_eq!(decoded.protocols.len(), 1);
}
