# NetPilot 配置参考

## 配置方式

NetPilot 支持三种配置输入方式：

| 模式 | 说明 |
|------|------|
| **Native JSON** | 通过 REST API 或配置文件的结构化 JSON |
| **BIRD2 兼容** | `bird.conf` 语法，由 `netpilot-birdconf` 解析器处理 |
| **gNMI Set** | 通过 gRPC gNMI 协议的 Replace 操作 |

## 顶层结构

```json
{
  "schema_version": 1,
  "identity": { "router_id": "192.0.2.1", "local_asn": 64512 },
  "hostname": "netpilot-edge-01",
  "tables": [],
  "protocols": [],
  "defines": [],
  "mpls_domains": [],
  "mpls_tables": [],
  "sr_prefix_sids": [],
  "sr_adjacency_sids": [],
  "srv6_locators": [],
  "srv6_sids": [],
  "snmp": {},
  "netconf": {},
  "pbr_rules": [],
  "vrrp_groups": [],
  "sbfd": {},
  "vnc_tunnels": [],
  "grpc_listen_addr": "0.0.0.0:50051"
}
```

## 完整字段参考

### RouterIdentity

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `router_id` | string | (必填) | IPv4 路由器 ID |
| `local_asn` | u32 | — | 自治系统号 |
| `router_id_from` | string | — | 从接口地址派生 router-id |

### TableConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `name` | string | "master" | 表名称 |
| `nettype` | NettypeDef | — | 网络类型：ip4/ip6/vpn4/vpn6/mpls/evpn 等 |
| `kernel_table` | u32 | 254 | Linux 内核路由表号 |
| `gc_threshold` | u32 | — | 垃圾回收阈值（路由数） |
| `gc_period_secs` | u32 | — | GC 周期间隔 |
| `sorted` | bool | — | 是否保持路由有序 |
| `trie` | bool | — | 使用 trie 结构加速查找 |
| `min_settle_time_secs` | u32 | — | 路由稳定最短时间 |
| `max_settle_time_secs` | u32 | — | 路由稳定最长时间 |

### NettypeDef

支持的 nettype 值：
`ip4`, `ip6`, `ip6-sadr`, `vpn4`, `vpn6`, `roa4`, `roa6`, `aspa`, `flow4`, `flow6`, `eth`, `mpls`, `evpn`, `neighbor`

### 协议通用字段

所有协议变体共享以下字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议实例名称 |
| `table` | string | 绑定的路由表 |
| `limits` | ChannelLimits | 通道导入/导出限制 |
| `import_keep_filtered` | bool | 保留被过滤的路由 |
| `rpki_reload` | bool | RPKI 变更时重载 |
| `passwords` | \[AuthPassword\] | 认证密码列表 |
| `password` | string | 简单密码 |
| `tx_class` | u8 | 发送 DSCP 分类 |
| `tx_priority` | u8 | 发送优先级 |
| `description` | string | 描述 |
| `mpls_channel` | MplsChannelConfig | MPLS 通道绑定 |

### ChannelLimits

| 字段 | 类型 | 说明 |
|------|------|------|
| `import_limit` | u32 | 最大导入路由数 |
| `import_limit_action` | LimitAction | 超限动作 (warn/block/restart/disable) |
| `receive_limit` | u32 | 最大接收路由数 |
| `receive_limit_action` | LimitAction | 超限动作 |
| `export_limit` | u32 | 最大导出路由数 |
| `export_limit_action` | LimitAction | 超限动作 |

### AuthPassword

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | u8 | 密码 ID |
| `password` | string | 密码字符串 |
| `generate_from` | string | 生成起始时间 (ISO 8601) |
| `generate_to` | string | 生成结束时间 |
| `accept_from` | string | 接受起始时间 |
| `accept_to` | string | 接受结束时间 |
| `algorithm` | AuthAlgorithm | 认证算法 |

### AuthAlgorithm

`keyed-md5`, `keyed-sha1`, `hmac-sha1`, `hmac-sha256`, `hmac-sha384`, `hmac-sha512`, `blake2s128`, `blake2s256`, `blake2b256`, `blake2b512`

---

## ProtocolConfig 变体

### Static

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `table` | string | 表 |
| `routes` | \[StaticRoute\] | 静态路由列表 |
| + 通用字段 | | |

#### StaticRoute

| 字段 | 类型 | 说明 |
|------|------|------|
| `prefix` | string | 目标前缀 (如 "10.0.0.0/8") |
| `next_hop` | string | 下一跳地址 |
| `blackhole` | bool | 黑洞路由 |
| `address_family` | AddressFamily | ipv4/ipv6/ipv4-labeled/ipv6-labeled |
| `nexthop_type` | StaticNexthopType | router/blackhole/unreachable/prohibit |
| `mpls_label` | u32 | MPLS 标签 |
| `igp_metric` | u32 | IGP 度量值 |

### BGP

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `table` | string | 主路由表 |
| `local_asn` | u32 | 本地 AS 号 |
| `neighbors` | \[BgpNeighbor\] | BGP 邻居列表 |
| `import_table` | string | 导入表 |
| `export_table` | string | 导出表 |
| `update_delay_secs` | u32 | 更新延迟 |
| `advertisement_delay_secs` | u32 | 通告延迟 |
| `coalesce_time_millis` | u32 | 合并时间 |
| `listen_range` | string | 监听范围 |
| `vrf` | string | VRF 名称 |
| `view` | string | BGP 视图 |
| `from_template` | string | 模板继承 |
| `aspa_downstream_check` | bool | ASPA 下游检查 |
| `aspa_upstream_check` | bool | ASPA 上游检查 |
| `bgp_ls` | BgpLsConfig | BGP-LS 配置 |
| `bgpsec` | BgpsecConfig | BGPsec 配置 |
| `flowspec` | \[BgpFlowspecConfig\] | Flowspec 规则 |
| + 通用字段 | | |

#### BgpNeighbor

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 邻居名称 |
| `remote_address` | string | 对端地址 |
| `remote_asn` | u32 | 对端 AS 号 |
| `address_families` | \[AddressFamily\] | 地址族 |
| `long_lived_graceful_restart` | bool | LLGR 启用 |
| `llgr_stale_time_secs` | u32 | LLGR 过期时间 |
| `graceful_restart_mode` | GrMode | GR 模式 (restarter/helper/disable) |
| `link_bandwidth` | LinkBandwidth | 链路带宽 |

#### BgpLsConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | bool | 启用 BGP-LS |
| `ls_identifier` | u32 | 链路状态标识符 |
| `instance_identifier` | u64 | 实例标识符 |
| `domain_id` | string | 域 ID |

#### BgpsecConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | bool | 启用 BGPsec |
| `key_path` | string | 私钥路径 |
| `algorithm` | BgpsecAlgorithm | rsa-sha256/ecdsa-p256-sha256/ecdsa-p384-sha384 |

#### BgpFlowspecConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | bool | 启用 Flowspec |
| `address_family` | AddressFamily | 地址族 |
| `rules` | \[FlowspecRule\] | 流规则 |

#### FlowspecRule

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 规则名称 |
| `action` | FlowspecAction | 动作：drop/rate-limit/redirect/remark/accept |
| `matches` | \[FlowspecMatch\] | 匹配条件 |
| `rate_limit_bps` | u64 | 限速值 |
| `remark` | string | DSCP 标记 |

#### FlowspecMatch (12 种匹配类型)

```json
{"type": "destination-prefix", "value": "10.0.0.0/8"}
{"type": "source-prefix", "value": "192.168.0.0/16"}
{"type": "ip-protocol", "values": [6, 17]}
{"type": "port", "values": [80, 443]}
{"type": "destination-port", "values": [53]}
{"type": "source-port", "values": [1024]}
{"type": "icmp-type", "values": [8]}
{"type": "icmp-code", "values": [0]}
{"type": "tcp-flags", "values": ["syn", "ack"]}
{"type": "packet-length", "min": 64, "max": 1500}
{"type": "dscp", "values": [46]}
{"type": "fragment", "values": ["dont-fragment", "first-fragment"]}
```

### OSPF

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `table` | string | 路由表 |
| `router_id` | string | 路由器 ID |
| `instance_id` | u8 | 实例 ID |
| `ecmp` | bool | 启用 ECMP |
| `ecmp_limit` | u32 | ECMP 最大路径数 |
| `areas` | \[OspfAreaConfig\] | 区域列表 |
| `stub_router` | bool | Stub 路由器模式 |
| `rfc1583_compat` | bool | RFC 1583 兼容 |
| `merge_external` | bool | 合并外部路由 |
| `tick_secs` | u32 | SPF 计算间隔 |
| `from_template` | string | 模板继承 |
| + 通用字段 | | |

#### OspfAreaConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `area_id` | string | 区域 ID (如 "0.0.0.0") |
| `nssa` | bool | NSSA 区域 |
| `nssa_translator` | bool | NSSA 转换器 |
| `nssa_translator_stability_secs` | u32 | 转换器稳定时间 |
| `default_cost` | u32 | 默认开销 |
| `default_cost2` | u32 | 默认开销 2 |

### IS-IS

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `table` | string | 路由表 |
| `area_addresses` | \[string\] | 区域地址 (如 "49.0001") |
| `system_id` | string | 系统 ID (如 "1920.0000.0001") |
| `levels` | \[IsisLevel\] | level1/level2/level12 |
| `interfaces` | \[IsisInterfaceConfig\] | 接口配置 |
| `sr_enabled` | bool | 启用 SR |
| + 通用字段 | | |

#### IsisInterfaceConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `interface` | string | 接口名称 |
| `levels` | \[IsisLevel\] | 级别 |
| `hello_interval_secs` | u32 | Hello 间隔 (默认 10s) |
| `hello_multiplier` | u8 | Hello 乘数 (默认 3) |
| `metric` | u32 | 度量值 |
| `passive` | bool | 被动模式 |
| `circuit_type` | CircuitType | level1/level2/level12 |
| `priority` | u8 | DIS 优先级 |
| `sr_adjacency_sid` | u32 | SR 邻接 SID |

### EIGRP

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `table` | string | 路由表 |
| `autonomous_system` | u32 | 自治系统号 |
| `router_id` | string | 路由器 ID |
| `interfaces` | \[EigrpInterfaceConfig\] | 接口配置 |
| `k_values` | KValues | K 值度量权重 |
| `maximum_paths` | u32 | 最大路径数 |
| `variance` | u32 | 非等价负载均衡方差 |
| + 通用字段 | | |

#### KValues

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `k1` | 1 | 带宽权重 |
| `k2` | 0 | 负载权重 |
| `k3` | 1 | 延迟权重 |
| `k4` | 0 | 可靠性权重 |
| `k5` | 0 | MTU 权重 |

#### EigrpInterfaceConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `interface` | string | 接口名称 |
| `hello_interval_secs` | u32 | Hello 间隔 |
| `hold_time_secs` | u32 | 保持时间 |
| `bandwidth_kbps` | u32 | 带宽 (kbps) |
| `delay_tens_of_microseconds` | u32 | 延迟 |
| `passive` | bool | 被动模式 |
| `split_horizon` | bool | 水平分割 |

### RIP

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `table` | string | 路由表 |
| `router_id` | string | 路由器 ID |
| `interfaces` | \[RipInterfaceConfig\] | 接口配置 |
| + 通用字段 | | |

#### RipInterfaceConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `interface` | string | 接口名称 |
| `metric` | u32 | 度量值 |
| `passive` | bool | 被动模式 |
| `split_horizon` | bool | 水平分割 |
| `poison_reverse` | bool | 毒化反转 |

### LDP

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `router_id` | string | 路由器 ID |
| `lsr_id` | string | LSR ID |
| `label_space_id` | u16 | 标签空间 ID |
| `transport_address` | string | 传输地址 |
| `interfaces` | \[LdpInterfaceConfig\] | 接口配置 |
| + 通用字段 | | |

#### LdpInterfaceConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `interface` | string | 接口名称 |
| `hello_interval_secs` | u32 | Hello 间隔 |
| `hold_time_secs` | u32 | 保持时间 |

### PIM

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 协议名称 |
| `table` | string | 路由表 |
| `router_id` | string | 路由器 ID |
| `interfaces` | \[PimInterfaceConfig\] | 接口配置 |
| `rp_addresses` | \[string\] | RP 地址列表 |
| `ssm_prefixes` | \[string\] | SSM 前缀列表 |
| + 通用字段 | | |

#### PimInterfaceConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `interface` | string | 接口名称 |
| `hello_interval_secs` | u32 | Hello 间隔 |
| `dr_priority` | u32 | DR 优先级 |
| `bfd_enabled` | bool | 启用 BFD |

---

## MPLS & Segment Routing

### MplsDomain

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 域名称 |
| `label_ranges` | \[MplsLabelRange\] | 标签范围 |
| `label_policy` | MplsLabelPolicy | static/per-prefix/aggregate/vrf |
| `max_label_stack_depth` | u8 | 最大标签栈深度 (1-32) |
| `sr_enabled` | bool | 启用 SR |
| `sr_global_block` | MplsLabelRange | SRGB 范围 |
| `static_bindings` | \[MplsStaticBinding\] | 静态绑定 |

### MplsLabelRange

| 字段 | 类型 | 说明 |
|------|------|------|
| `low` | u32 | 起始标签 (min 16) |
| `high` | u32 | 结束标签 (max 1,048,575) |

### MplsStaticBinding

| 字段 | 类型 | 说明 |
|------|------|------|
| `prefix` | string | 前缀 |
| `label` | u32 | 标签 |

### MplsTableConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 表名称 |
| `domain` | string | MPLS 域引用 |
| `gc_threshold` | u32 | GC 阈值 |
| `gc_period_secs` | u32 | GC 间隔 |
| `sorted` | bool | 有序 |
| `min_settle_time_secs` | u32 | 稳定时间 |
| `max_settle_time_secs` | u32 | 最大稳定时间 |

### MplsChannelConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `table` | string | MPLS 表引用 |
| `import_limit` | u32 | 导入限制 |
| `import_limit_action` | LimitAction | 超限动作 |
| `export_limit` | u32 | 导出限制 |
| `export_limit_action` | LimitAction | 超限动作 |
| `import_keep_filtered` | bool | 保留过滤 |

### SR Prefix-SID (SrPrefixSidConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `prefix` | string | 前缀 |
| `domain` | string | MPLS 域引用 |
| `sid_type` | SrSidType | absolute/索引 |
| `flags` | SrPrefixSidFlags | N-flag-clear/PHP/explicit-null |

### SrPrefixSidFlags

| 字段 | 类型 | 说明 |
|------|------|------|
| `n_flag_clear` | bool | 清除 N-flag |
| `php` | bool | Penultimate Hop Popping |
| `explicit_null` | bool | 显式空标签 |

### SR Adjacency-SID (SrAdjacencySidConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `interface` | string | 接口 |
| `neighbor` | string | 邻居 |
| `domain` | string | MPLS 域 |
| `sid_type` | SrAdjSidType | absolute/dynamic |
| `protected` | bool | 保护路径 |

### SRv6 Locator (Srv6LocatorConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 定位器名称 |
| `prefix` | string | IPv6 前缀 |
| `block_len` | u8 | 块长度 |
| `node_len` | u8 | 节点长度 |
| `function_len` | u8 | 功能长度 |

### Srv6 SID (Srv6SidConfig)

5 种 behavior 类型：
- **End**：`{"behavior": "end", "name": "sid1", "locator": "loc1", "function": 1}`
- **End.X**：同上 + `"interface": "eth0", "nexthop": "2001:db8::1"`
- **End.T**：同上 + `"vrf": "vrf-red"`
- **End.DT4**：同上 + `"vrf": "vrf-red"`
- **End.DT6**：同上 + `"vrf": "vrf-red"`

---

## 安全功能

### SNMP (SnmpConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | bool | 启用 SNMP |
| `listen_addr` | string | 监听地址 |
| `community` | string | 只读 community |
| `location` | string | 设备位置 |
| `contact` | string | 管理员联系 |
| `engine_id` | string | SNMP 引擎 ID |

### NETCONF (NetconfConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | bool | 启用 NETCONF |
| `listen_addr` | string | 监听地址 |
| `yang_modules` | \[YangModelConfig\] | YANG 模块 |
| `username` | string | 用户名 |
| `password` | string | 密码 |

### SBFD (SbfdConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | bool | 启用 SBFD |
| `reflector` | bool | 反射器模式 |
| `discriminator` | u32 | 本地鉴别器 |
| `min_tx_interval_millis` | u32 | 最小发送间隔 |
| `min_rx_interval_millis` | u32 | 最小接收间隔 |
| `multiplier` | u8 | 检测倍数 |

### TLS (gRPC)

| 字段 | 类型 | 说明 |
|------|------|------|
| `grpc_listen_addr` | string | gRPC 监听地址 |
| `grpc_tls_cert_path` | string | TLS 证书路径 |
| `grpc_tls_key_path` | string | TLS 密钥路径 |

---

## 转发功能

### PBR (PbrConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 规则名称 |
| `rules` | \[PbrRule\] | PBR 规则 |

#### PbrRule

| 字段 | 类型 | 说明 |
|------|------|------|
| `seq` | u32 | 顺序号 |
| `action` | PbrAction | permit/deny |
| `match_prefix` | string | 匹配前缀 |
| `match_src_port` | u16 | 匹配源端口 |
| `match_dst_port` | u16 | 匹配目标端口 |
| `match_protocol` | u8 | 匹配协议 |
| `set_next_hop` | string | 设置下一跳 |
| `set_interface` | string | 设置出口接口 |

### VRRP (VrrpConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 组名称 |
| `interface` | string | 接口 |
| `vrid` | u8 | 虚拟路由器 ID |
| `priority` | u8 | 优先级 (默认 100) |
| `virtual_addresses` | \[string\] | 虚拟 IP 地址 |
| `advertisement_interval_secs` | u32 | 通告间隔 |
| `preempt` | bool | 抢占模式 |

### VNC (VncConfig)

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 隧道名称 |
| `nve_ip` | string | NVE IP |
| `vni` | u32 | VNI |
| `multicast_group` | string | 多播组 |
| `head_end_replication` | bool | 头端复制 |
| `flood_list` | \[string\] | 泛洪列表 |
| `description` | string | 描述 |

---

## 参考示例

完整示例配置：`configs/example-bgp.json` — BGP + Static + MPLS + SR 的综合配置。
