#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Nettype {
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
    EvpnEad,
    EvpnMac,
    EvpnImet,
    EvpnEs,
    Neighbor,
}

impl Nettype {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "NET_IP4" => Some(Self::Ip4),
            "NET_IP6" => Some(Self::Ip6),
            "NET_IP6_SADR" => Some(Self::Ip6Sadr),
            "NET_VPN4" => Some(Self::Vpn4),
            "NET_VPN6" => Some(Self::Vpn6),
            "NET_ROA4" => Some(Self::Roa4),
            "NET_ROA6" => Some(Self::Roa6),
            "NET_ASPA" => Some(Self::Aspa),
            "NET_FLOW4" => Some(Self::Flow4),
            "NET_FLOW6" => Some(Self::Flow6),
            "NET_ETH" => Some(Self::Eth),
            "NET_MPLS" => Some(Self::Mpls),
            "NET_EVPN" => Some(Self::Evpn),
            "NET_EVPN_EAD" => Some(Self::EvpnEad),
            "NET_EVPN_MAC" => Some(Self::EvpnMac),
            "NET_EVPN_IMET" => Some(Self::EvpnImet),
            "NET_EVPN_ES" => Some(Self::EvpnEs),
            "NET_NEIGHBOR" => Some(Self::Neighbor),
            _ => None,
        }
    }
}
