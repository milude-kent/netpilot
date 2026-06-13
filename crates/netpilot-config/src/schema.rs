use netpilot_filter::nettype::Nettype;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RoutePlaneConfig {
    pub schema_version: u32,
    pub identity: RouterIdentity,
    pub tables: Vec<TableConfig>,
    pub protocols: Vec<ProtocolConfig>,
    pub hostname: Option<String>,
    pub defines: Option<Vec<ConstantDef>>,
    pub cli_sockets: Option<Vec<CliSocketConfig>>,
    pub watchdog_warning_secs: Option<u32>,
    pub watchdog_timeout_secs: Option<u32>,
    pub debug_latency: Option<bool>,
    pub debug_latency_limit_micros: Option<u64>,
    pub debug_protocols: Option<String>,
    pub debug_channels: Option<String>,
    pub debug_tables: Option<String>,
    pub debug_commands: Option<u8>,
    pub timeformat_route: Option<String>,
    pub timeformat_protocol: Option<String>,
    pub timeformat_base: Option<String>,
    pub timeformat_log: Option<String>,
    pub mpls_domains: Option<Vec<MplsDomain>>,
    pub mpls_tables: Option<Vec<MplsTableConfig>>,
    pub srv6_locators: Option<Vec<Srv6LocatorConfig>>,
    pub sr_prefix_sids: Option<Vec<SrPrefixSidConfig>>,
    pub sr_adjacency_sids: Option<Vec<SrAdjacencySidConfig>>,
    pub srv6_sids: Option<Vec<Srv6SidConfig>>,
    pub grpc_listen_addr: Option<String>,
    pub grpc_tls_cert_path: Option<String>,
    pub grpc_tls_key_path: Option<String>,
    pub snmp: Option<SnmpConfig>,
    pub netconf: Option<NetconfConfig>,
    pub pbr_rules: Option<Vec<PbrConfig>>,
    pub vrrp_groups: Option<Vec<VrrpConfig>>,
    pub sbfd: Option<SbfdConfig>,
    pub vnc_tunnels: Option<Vec<VncConfig>>,
    /// Authentication / TLS settings for the REST + gRPC control plane.
    /// When `None` the control plane is unauthenticated and uses plain HTTP.
    pub auth: Option<AuthConfig>,
}

impl Default for RoutePlaneConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            identity: RouterIdentity::default(),
            tables: vec![TableConfig {
                name: "master".to_string(),
                nettype: None,
                kernel_table: Some(254),
                gc_threshold: None,
                gc_period_secs: None,
                sorted: None,
                trie: None,
                min_settle_time_secs: None,
                max_settle_time_secs: None,
            }],
            protocols: Vec::new(),
            hostname: None,
            defines: None,
            cli_sockets: None,
            watchdog_warning_secs: None,
            watchdog_timeout_secs: None,
            debug_latency: None,
            debug_latency_limit_micros: None,
            debug_protocols: None,
            debug_channels: None,
            debug_tables: None,
            debug_commands: None,
            timeformat_route: None,
            timeformat_protocol: None,
            timeformat_base: None,
            timeformat_log: None,
            mpls_domains: None,
            mpls_tables: None,
            srv6_locators: None,
            sr_prefix_sids: None,
            sr_adjacency_sids: None,
            srv6_sids: None,
            grpc_listen_addr: None,
            grpc_tls_cert_path: None,
            grpc_tls_key_path: None,
            snmp: None,
            netconf: None,
            pbr_rules: None,
            vrrp_groups: None,
            sbfd: None,
            vnc_tunnels: None,
            auth: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouterIdentity {
    pub router_id: String,
    pub local_asn: Option<u32>,
    pub router_id_from: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TableConfig {
    pub name: String,
    pub nettype: Option<NettypeDef>,
    pub kernel_table: Option<u32>,
    pub gc_threshold: Option<u32>,
    pub gc_period_secs: Option<u32>,
    pub sorted: Option<bool>,
    pub trie: Option<bool>,
    pub min_settle_time_secs: Option<u32>,
    pub max_settle_time_secs: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
// The variants intentionally carry every protocol's configuration inline so the
// schema is one round-trip from JSON to a fully-typed view. The enum is only
// ever held inside `Vec<ProtocolConfig>`, which heap-allocates the buffer, so
// the per-variant size delta does not propagate to a hot stack path.
#[allow(clippy::large_enum_variant)]
pub enum ProtocolConfig {
    Static {
        name: String,
        table: String,
        routes: Vec<StaticRoute>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
    Bgp {
        name: String,
        table: String,
        local_asn: u32,
        neighbors: Vec<BgpNeighbor>,
        import_table: Option<String>,
        export_table: Option<String>,
        update_delay_secs: Option<u32>,
        advertisement_delay_secs: Option<u32>,
        coalesce_time_millis: Option<u32>,
        listen_range: Option<String>,
        vrf: Option<String>,
        view: Option<String>,
        from_template: Option<String>,
        aspa_downstream_check: Option<bool>,
        aspa_upstream_check: Option<bool>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
        bgp_ls: Option<BgpLsConfig>,
        bgpsec: Option<BgpsecConfig>,
        flowspec: Option<Vec<BgpFlowspecConfig>>,
    },
    Ospf {
        name: String,
        table: String,
        router_id: Option<String>,
        instance_id: Option<u8>,
        ecmp: Option<bool>,
        ecmp_limit: Option<u32>,
        areas: Vec<OspfAreaConfig>,
        stub_router: Option<bool>,
        rfc1583_compat: Option<bool>,
        merge_external: Option<bool>,
        tick_secs: Option<u32>,
        from_template: Option<String>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
    Isis {
        name: String,
        table: String,
        area_addresses: Vec<String>,
        system_id: String,
        levels: Vec<IsisLevel>,
        interfaces: Vec<IsisInterfaceConfig>,
        sr_enabled: Option<bool>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
    Eigrp {
        name: String,
        table: String,
        autonomous_system: u32,
        router_id: String,
        interfaces: Vec<EigrpInterfaceConfig>,
        k_values: Option<KValues>,
        maximum_paths: Option<u32>,
        variance: Option<u32>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        rpki_reload: Option<bool>,
        passwords: Option<Vec<AuthPassword>>,
        password: Option<String>,
        tx_class: Option<u8>,
        tx_priority: Option<u8>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
    Ldp {
        name: String,
        router_id: String,
        lsr_id: String,
        label_space_id: Option<u16>,
        transport_address: Option<String>,
        interfaces: Vec<LdpInterfaceConfig>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
    Pim {
        name: String,
        table: String,
        router_id: String,
        interfaces: Vec<PimInterfaceConfig>,
        rp_addresses: Option<Vec<String>>,
        ssm_prefixes: Option<Vec<String>>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
    Rip {
        name: String,
        table: String,
        router_id: String,
        interfaces: Vec<RipInterfaceConfig>,
        limits: Option<ChannelLimits>,
        import_keep_filtered: Option<bool>,
        description: Option<String>,
        mpls_channel: Option<MplsChannelConfig>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IsisLevel {
    Level1,
    Level2,
    Level12,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct IsisInterfaceConfig {
    pub interface: String,
    pub levels: Vec<IsisLevel>,
    pub hello_interval_secs: Option<u32>,
    pub hello_multiplier: Option<u8>,
    pub metric: Option<u32>,
    pub passive: Option<bool>,
    pub circuit_type: Option<CircuitType>,
    pub priority: Option<u8>,
    pub sr_adjacency_sid: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CircuitType {
    Level1,
    Level2,
    Level12,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct KValues {
    pub k1: Option<u8>,
    pub k2: Option<u8>,
    pub k3: Option<u8>,
    pub k4: Option<u8>,
    pub k5: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EigrpInterfaceConfig {
    pub interface: String,
    pub hello_interval_secs: Option<u32>,
    pub hold_time_secs: Option<u32>,
    pub bandwidth_kbps: Option<u32>,
    pub delay_tens_of_microseconds: Option<u32>,
    pub passive: Option<bool>,
    pub split_horizon: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ChannelLimits {
    pub import_limit: Option<u32>,
    pub import_limit_action: Option<LimitAction>,
    pub receive_limit: Option<u32>,
    pub receive_limit_action: Option<LimitAction>,
    pub export_limit: Option<u32>,
    pub export_limit_action: Option<LimitAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LimitAction {
    Warn,
    Block,
    Restart,
    Disable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuthPassword {
    pub id: Option<u8>,
    pub password: String,
    pub generate_from: Option<String>,
    pub generate_to: Option<String>,
    pub accept_from: Option<String>,
    pub accept_to: Option<String>,
    pub algorithm: Option<AuthAlgorithm>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthAlgorithm {
    KeyedMd5,
    KeyedSha1,
    HmacSha1,
    HmacSha256,
    HmacSha384,
    HmacSha512,
    Blake2s128,
    Blake2s256,
    Blake2b256,
    Blake2b512,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GrMode {
    Restarter,
    Helper,
    Disable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkBandwidth {
    IeeeFloat(f64),
    Uint32(u32),
}

impl Eq for LinkBandwidth {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConstantDef {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StaticNexthopType {
    Router,
    Blackhole,
    Unreachable,
    Prohibit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StaticRoute {
    pub prefix: String,
    pub next_hop: Option<String>,
    pub blackhole: bool,
    pub address_family: AddressFamily,
    pub nexthop_type: Option<StaticNexthopType>,
    pub mpls_label: Option<u32>,
    pub igp_metric: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BgpNeighbor {
    pub name: String,
    pub remote_address: String,
    pub remote_asn: u32,
    pub address_families: Vec<AddressFamily>,
    pub long_lived_graceful_restart: Option<bool>,
    pub llgr_stale_time_secs: Option<u32>,
    pub graceful_restart_mode: Option<GrMode>,
    pub link_bandwidth: Option<LinkBandwidth>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
    Ipv4Labeled,
    Ipv6Labeled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NettypeDef {
    Ip4,
    Ip6,
    Ip6Sadr,
    Vpn4,
    Vpn6,
    Roa4,
    Roa6,
    Aspa,
    Flow4,
    Flow6,
    Eth,
    Mpls,
    Evpn,
    Neighbor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OspfAreaConfig {
    pub area_id: String,
    pub nssa: Option<bool>,
    pub nssa_translator: Option<bool>,
    pub nssa_translator_stability_secs: Option<u32>,
    pub default_cost: Option<u32>,
    pub default_cost2: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TemplateRef {
    pub template_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CliSocketConfig {
    pub path: String,
    pub restrict: Option<bool>,
}

// ── MPLS Domain ──────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsDomain {
    pub name: String,
    pub label_ranges: Vec<MplsLabelRange>,
    pub label_policy: Option<MplsLabelPolicy>,
    pub max_label_stack_depth: Option<u8>,
    pub sr_enabled: Option<bool>,
    pub sr_global_block: Option<MplsLabelRange>,
    pub static_bindings: Option<Vec<MplsStaticBinding>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsLabelRange {
    pub low: u32,
    pub high: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MplsLabelPolicy {
    Static,
    PerPrefix,
    Aggregate,
    Vrf,
}

// ── MPLS Static Binding ──────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsStaticBinding {
    pub prefix: String,
    pub label: u32,
}

// ── MPLS Table ───────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsTableConfig {
    pub name: String,
    pub domain: String,
    pub gc_threshold: Option<u32>,
    pub gc_period_secs: Option<u32>,
    pub sorted: Option<bool>,
    pub min_settle_time_secs: Option<u32>,
    pub max_settle_time_secs: Option<u32>,
}

// ── MPLS Channel ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MplsChannelConfig {
    pub table: String,
    pub import_limit: Option<u32>,
    pub import_limit_action: Option<LimitAction>,
    pub export_limit: Option<u32>,
    pub export_limit_action: Option<LimitAction>,
    pub import_keep_filtered: Option<bool>,
}

// ── SRv6 Locator (schema-only seed) ──────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Srv6LocatorConfig {
    pub name: String,
    pub prefix: String,
    pub block_len: Option<u8>,
    pub node_len: Option<u8>,
    pub function_len: Option<u8>,
}

impl From<NettypeDef> for Nettype {
    fn from(def: NettypeDef) -> Self {
        match def {
            NettypeDef::Ip4 => Nettype::Ip4,
            NettypeDef::Ip6 => Nettype::Ip6,
            NettypeDef::Ip6Sadr => Nettype::Ip6Sadr,
            NettypeDef::Vpn4 => Nettype::Vpn4,
            NettypeDef::Vpn6 => Nettype::Vpn6,
            NettypeDef::Roa4 => Nettype::Roa4,
            NettypeDef::Roa6 => Nettype::Roa6,
            NettypeDef::Aspa => Nettype::Aspa,
            NettypeDef::Flow4 => Nettype::Flow4,
            NettypeDef::Flow6 => Nettype::Flow6,
            NettypeDef::Eth => Nettype::Eth,
            NettypeDef::Mpls => Nettype::Mpls,
            NettypeDef::Evpn => Nettype::Evpn,
            NettypeDef::Neighbor => Nettype::Neighbor,
        }
    }
}

// ── SR-MPLS ────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SrPrefixSidConfig {
    pub prefix: String,
    pub domain: String,
    pub sid_type: SrSidType,
    pub flags: SrPrefixSidFlags,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum SrSidType {
    Absolute(u32),
    Index(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SrPrefixSidFlags {
    pub n_flag_clear: Option<bool>,
    pub php: Option<bool>,
    pub explicit_null: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SrAdjacencySidConfig {
    pub interface: String,
    pub neighbor: String,
    pub domain: String,
    pub sid_type: SrAdjSidType,
    pub protected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum SrAdjSidType {
    Absolute(u32),
    Dynamic,
}

// ── BGP-LS ──────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BgpLsConfig {
    pub enabled: bool,
    pub ls_identifier: Option<u32>,
    pub instance_identifier: Option<u64>,
    pub domain_id: Option<String>,
}

// ── BGPsec ──────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BgpsecConfig {
    pub enabled: bool,
    pub key_path: Option<String>,
    pub algorithm: Option<BgpsecAlgorithm>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BgpsecAlgorithm {
    RsaSha256,
    EcdsaP256Sha256,
    EcdsaP384Sha384,
}

// ── BGP Flowspec ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BgpFlowspecConfig {
    pub enabled: bool,
    pub address_family: AddressFamily,
    pub rules: Vec<FlowspecRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FlowspecRule {
    pub name: String,
    pub action: FlowspecAction,
    pub matches: Vec<FlowspecMatch>,
    pub rate_limit_bps: Option<u64>,
    pub remark: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlowspecAction {
    Drop,
    RateLimit,
    Redirect { next_hop: String },
    Remark,
    Accept,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum FlowspecMatch {
    DestinationPrefix { value: String },
    SourcePrefix { value: String },
    IpProtocol { values: Vec<u8> },
    Port { values: Vec<u16> },
    DestinationPort { values: Vec<u16> },
    SourcePort { values: Vec<u16> },
    IcmpType { values: Vec<u8> },
    IcmpCode { values: Vec<u8> },
    TcpFlags { values: Vec<String> },
    PacketLength { min: u16, max: u16 },
    Dscp { values: Vec<u8> },
    Fragment { values: Vec<String> },
}

// ── SRv6 ───────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "kebab-case")]
pub enum Srv6SidConfig {
    End {
        name: String,
        locator: String,
        function: u32,
    },
    EndX {
        name: String,
        locator: String,
        function: u32,
        interface: String,
        nexthop: String,
    },
    EndT {
        name: String,
        locator: String,
        function: u32,
        vrf: String,
    },
    EndDT4 {
        name: String,
        locator: String,
        function: u32,
        vrf: String,
    },
    EndDT6 {
        name: String,
        locator: String,
        function: u32,
        vrf: String,
    },
}

// ── SNMP (#312) ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SnmpConfig {
    pub enabled: bool,
    pub listen_addr: Option<String>, // default "0.0.0.0:161"
    pub community: Option<String>,   // read-only community
    pub location: Option<String>,
    pub contact: Option<String>,
    pub engine_id: Option<String>,
}

// ── YANG (#313) ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct YangModelConfig {
    pub name: String,
    pub namespace: String,
    pub prefix: String,
    pub revision: Option<String>,
    pub schema_path: Option<String>,
}

// ── NETCONF (#314) ──────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NetconfConfig {
    pub enabled: bool,
    pub listen_addr: Option<String>, // default "0.0.0.0:830"
    pub yang_modules: Option<Vec<YangModelConfig>>,
    pub username: Option<String>,
    pub password: Option<String>,
}

// ── LDP (#303) ──────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LdpInterfaceConfig {
    pub interface: String,
    pub hello_interval_secs: Option<u32>,
    pub hold_time_secs: Option<u32>,
}

// ── PIM (#302) ──────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PimInterfaceConfig {
    pub interface: String,
    pub hello_interval_secs: Option<u32>,
    pub dr_priority: Option<u32>,
    pub bfd_enabled: Option<bool>,
}

// ── RIP (#317) ──────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RipInterfaceConfig {
    pub interface: String,
    pub metric: Option<u32>,
    pub passive: Option<bool>,
    pub split_horizon: Option<bool>,
    pub poison_reverse: Option<bool>,
}

// ── PBR (#306) ──────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PbrRule {
    pub seq: u32,
    pub action: PbrAction,
    pub match_prefix: Option<String>,
    pub match_src_port: Option<u16>,
    pub match_dst_port: Option<u16>,
    pub match_protocol: Option<u8>,
    pub set_next_hop: Option<String>,
    pub set_interface: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PbrConfig {
    pub name: String,
    pub rules: Vec<PbrRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PbrAction {
    Permit,
    Deny,
}

// ── VRRP (#307) ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct VrrpConfig {
    pub name: String,
    pub interface: String,
    pub vrid: u8,
    pub priority: Option<u8>,
    pub virtual_addresses: Vec<String>,
    pub advertisement_interval_secs: Option<u32>,
    pub preempt: Option<bool>,
}

// ── SBFD (#308) ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SbfdConfig {
    pub enabled: bool,
    pub reflector: Option<bool>,
    pub discriminator: Option<u32>,
    pub min_tx_interval_millis: Option<u32>,
    pub min_rx_interval_millis: Option<u32>,
    pub multiplier: Option<u8>,
}

// ── VNC (#320) ──────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct VncConfig {
    pub name: String,
    pub nve_ip: String,
    pub vni: u32,
    pub multicast_group: Option<String>,
    pub head_end_replication: Option<bool>,
    pub flood_list: Option<Vec<String>>,
    pub description: Option<String>,
}

// ── Auth (C1) ────────────────────────────────────────────────

/// Authentication / TLS configuration for the control plane (REST + gRPC).
///
/// * `bearer_secret` is the HMAC-SHA256 key used to sign and verify bearer
///   tokens of the form `<exp_unix>.<hex_hmac>`. When `None`, bearer
///   authentication is disabled and the daemon falls back to a
///   "no authentication" mode that should only be used in trusted
///   environments.
/// * `tls_cert_path` / `tls_key_path` enable TLS on the REST listener when
///   both are present.
/// * `tls_client_ca_path` enables mTLS (client certificate verification)
///   when present.
/// * `allowed_spiffe_ids` is an optional allowlist of client certificate
///   URI SANs that are accepted by mTLS.
/// * `unauthed_paths` is the list of routes that bypass the bearer
///   middleware (defaults to `/health` and `/metrics`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuthConfig {
    /// Bearer token secret (HMAC-SHA256 key). If `None`, bearer auth is
    /// disabled.
    pub bearer_secret: Option<String>,
    /// TLS server certificate PEM path.
    pub tls_cert_path: Option<PathBuf>,
    /// TLS server private key PEM path.
    pub tls_key_path: Option<PathBuf>,
    /// TLS client CA bundle PEM path (for mTLS verify).
    pub tls_client_ca_path: Option<PathBuf>,
    /// Allowed SPIFFE IDs (validates against client cert URI SAN).
    #[serde(default)]
    pub allowed_spiffe_ids: Vec<String>,
    /// Routes that bypass auth (default: `/health`, `/metrics`).
    #[serde(default = "default_unauthed_paths")]
    pub unauthed_paths: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            bearer_secret: None,
            tls_cert_path: None,
            tls_key_path: None,
            tls_client_ca_path: None,
            allowed_spiffe_ids: Vec::new(),
            unauthed_paths: default_unauthed_paths(),
        }
    }
}

fn default_unauthed_paths() -> Vec<String> {
    vec!["/health".into(), "/metrics".into()]
}
