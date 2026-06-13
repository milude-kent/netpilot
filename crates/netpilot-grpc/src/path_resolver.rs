use crate::gnmi::{Path, PathValue};
use crate::GrpcAppState;

pub fn resolve(state: &GrpcAppState, path: &Path) -> Option<PathValue> {
    let path_str = path.elem.join("/");
    let value: Option<Vec<u8>> = match path_str.as_str() {
        "netpilot/config/running" => {
            let store = state.config_store.try_read().ok()?;
            serde_json::to_vec(store.running()).ok()
        }
        "netpilot/state/health" => {
            Some(br#"{"status":"SERVING"}"#.to_vec())
        }
        "netpilot/state/protocols" => {
            let store = state.config_store.try_read().ok()?;
            let config = store.running();
            let names: Vec<&str> = config.protocols.iter().map(|p| match p {
                netpilot_config::ProtocolConfig::Static { name, .. } => name.as_str(),
                netpilot_config::ProtocolConfig::Bgp { name, .. } => name.as_str(),
                netpilot_config::ProtocolConfig::Ospf { name, .. } => name.as_str(),
                netpilot_config::ProtocolConfig::Isis { name, .. } => name.as_str(),
                netpilot_config::ProtocolConfig::Eigrp { name, .. } => name.as_str(),
                netpilot_config::ProtocolConfig::Ldp { name, .. } => name.as_str(),
                netpilot_config::ProtocolConfig::Pim { name, .. } => name.as_str(),
            }).collect();
            serde_json::to_vec(&names).ok()
        }
        "netpilot/state/mpls/domains" => {
            let store = state.config_store.try_read().ok()?;
            let config = store.running();
            serde_json::to_vec(config.mpls_domains.as_deref().unwrap_or(&[])).ok()
        }
        "netpilot/state/sr/prefix-sids" => {
            let store = state.config_store.try_read().ok()?;
            let config = store.running();
            serde_json::to_vec(config.sr_prefix_sids.as_deref().unwrap_or(&[])).ok()
        }
        _ => None,
    };
    value.map(|v| PathValue { path: Some(path.clone()), value: v })
}
