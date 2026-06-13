# NetPilot 介绍文档

## 概述

NetPilot 是一个 Rust 编写的路由守护进程平台，目标是成为 **BIRD 2.x 的完整运行替代方案**。它提供了 BIRD2 兼容的配置语法、结构化配置管理、现代管理接口（REST API、gRPC/gNMI、Web UI、CLI），以及完整的 BGP/OSPF/IS-IS/EIGRP/RIP 协议支持。

### 设计理念

- **兼容性优先**：支持直接解析 `bird.conf` 配置语法，现有 BIRD2 用户可平滑迁移
- **结构化配置**：候选/运行配置、diff、commit、confirmed-commit、rollback、revision 追踪
- **微内核架构**：Rust 微内核 + 协议 actor，每个协议作为独立 tokio 任务运行
- **多面管理**：REST API、gRPC/gNMI、交互式 CLI socket、React Web 仪表板

## 架构

```
┌──────────────────────────────────────────────────────────┐
│                    管理面                                  │
│  Web UI (:8080/)  │  REST API (:8080/api/)  │  CLI socket │
│  gRPC + gNMI (:50051)  │  SSE 事件流 (/api/events)       │
├──────────────────────────────────────────────────────────┤
│                    配置引擎                                │
│  BIRD2 Parser → AST → Structured Config → Validate → Diff │
│  ConfigStore: Candidate/Running/Revisions/Rollback        │
├──────────────────────────────────────────────────────────┤
│                    路由核心                                │
│  ProtocolSupervisor → 事件广播                             │
│  RIB (RouteTable) → best-route 选择 → ECMP → NextHop      │
│  Channel Engine → import/export 过滤器                     │
├──────────────────────────────────────────────────────────┤
│                    协议 Actors (tokio tasks)               │
│  BGP │ OSPF │ IS-IS │ EIGRP │ RIP │ LDP │ PIM │ BFD       │
├──────────────────────────────────────────────────────────┤
│                    数据面                                  │
│  Kernel (netlink) → Linux FIB                             │
│  MPLS Label Pool → SR-MPLS/SRv6                          │
└──────────────────────────────────────────────────────────┘
```

## 快速开始

### 启动守护进程

```bash
# 构建并启动
./scripts/start.sh

# 或手动启动
cargo run -p netpilotd
```

守护进程启动后，以下接口将可用：
- **Web UI**：http://127.0.0.1:8080/
- **REST API**：http://127.0.0.1:8080/api/
- **gRPC**：127.0.0.1:50051
- **健康检查**：http://127.0.0.1:8080/health

### 提交第一个配置

```bash
# 1. 查看当前运行配置
curl http://127.0.0.1:8080/api/config/running | jq

# 2. 提交候选配置
curl -X PUT http://127.0.0.1:8080/api/config/candidate \
  -H "Content-Type: application/json" \
  -d @configs/example-bgp.json

# 3. 查看差异
curl http://127.0.0.1:8080/api/config/diff | jq

# 4. 提交
curl -X POST http://127.0.0.1:8080/api/config/commit \
  -H "Content-Type: application/json" \
  -d '{"author":"admin","note":"initial config"}'
```

### 运行冒烟测试

```bash
./scripts/smoke-test.sh
```

## 支持的协议

| 协议 | Crate | 功能 |
|------|-------|------|
| **BGP** | netpilot-proto-bgp | TCP session (179), OPEN/KEEPALIVE/UPDATE, multi-AS, LLGR, BGP-LU |
| **OSPF** | netpilot-proto-ospf | OSPFv2/v3, 区域, LSDB, SPF, NSSA, ECMP |
| **IS-IS** | netpilot-proto-isis | L1/L2, 邻接 FSM, Dijkstra SPF, LSP 数据库, TLV 解析 |
| **EIGRP** | netpilot-proto-eigrp | DUAL 算法, 邻居表, 可行后继, 复合度量 |
| **RIP** | netpilot-proto-rip | RIPv2, 距离矢量, 水平分割, 毒化反转 |
| **LDP** | netpilot-proto-ldp | 标签分发, FEC 绑定, MPLS 标签池 |
| **PIM** | netpilot-proto-pim | PIM-SM, 多播组加入/离开, RP 配置 |
| **BFD** | netpilot-proto-bfd | 双向转发检测, Down/Init/Up FSM |
| **Static** | netpilot-config | 静态路由, blackhole/unreachable/prohibit, MPLS label |
| **Direct** | netpilot-kernel | 直连路由发现, 接口地址 |
| **RPKI** | netpilot-proto-rpki | RTR client, ROA 验证, ASPA 检查 |

## 管理接口

### REST API

| 方法 | 端点 | 说明 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/api/config/running` | 获取运行配置 |
| GET | `/api/config/candidate` | 获取候选配置 |
| PUT | `/api/config/candidate` | 提交候选配置 |
| GET | `/api/config/diff` | 查看 running↔candidate 差异 |
| POST | `/api/config/commit` | 提交候选配置 |
| POST | `/api/config/rollback` | 回滚到指定 revision |
| GET | `/api/events` | SSE 事件流（实时路由/状态变更） |

### gRPC / gNMI

gNMI 服务运行在端口 50051，支持：
- **Capabilities** — 返回支持的模型和编码
- **Get** — 按路径查询配置/状态
- **Set** — 通过 replace 修改候选配置
- **Subscribe** — 流式遥测（ONCE、STREAM、POLL）

示例 gNMI 路径：
- `/netpilot/config/running` — 运行配置
- `/netpilot/state/health` — 健康状态
- `/netpilot/state/protocols` — 协议列表
- `/netpilot/state/mpls/domains` — MPLS 域状态
- `/netpilot/state/sr/prefix-sids` — SR 前缀 SID

### CLI 命令

交互式 CLI 提供以下命令族：
- `show status | protocols | interfaces | route | symbols | bfd | rpki | memory`
- `show mpls labels | sr prefix-sids | srv6 sids`
- `show isis topology | adjacencies | database`
- `show eigrp neighbors | topology | routes`
- `show bgp link-state | flowspec`
- `show snmp | vrrp | pbr | ldp neighbors | pim neighbors`
- `configure [soft] [check] [confirm | undo] [timeout <n>]`
- `enable | disable | restart | reload <name>`
- `eval <expr>` — 过滤器求值
- `dump <kind> <file>` — 状态导出
- `debug <target> <flags>` — 调试

## 过滤器语言

NetPilot 实现了 BIRD2 兼容的过滤器语言，包括：

**数据类型**：bool、int、string、prefix、ip、bgppath、bgpmask、clist、eclist、lclist、bytestring、mac、rd

**控制结构**：if/else、case、for loop、function

**内置函数**：print/printn、defined/unset、from_hex、len、accept/reject

**路由属性**：preference、metric、source、net、gw、as_path、communities、mpls_label、igp_metric

**过滤器 VM**：完整解释器，支持算术、比较、条件判断和自定义函数

## 项目结构

```
crates/
  netpilot-config/       ← 配置 schema、验证、diff、存储
  netpilot-filter/       ← BIRD2 过滤器语言 + VM 解释器
  netpilot-protocol/     ← ProtocolActor trait + Supervisor
  netpilot-rib/          ← RIB：路由表 + 选择 + ECMP
  netpilot-channel/      ← 通道引擎：import/export 过滤器
  netpilot-kernel/       ← netlink 路由 + 接口监听
  netpilot-io/           ← BGP TCP + 原始 socket 传输
  netpilot-auth/         ← 协议认证 (HMAC/MD5)
  netpilot-birdconf/     ← BIRD2 配置解析器
  netpilot-grpc/         ← gNMI/gRPC 服务
  netpilot-proto-isis/   ← IS-IS 协议
  netpilot-proto-eigrp/  ← EIGRP 协议
  netpilot-proto-ospf/   ← OSPF 协议
  netpilot-proto-bgp/    ← BGP 协议
  netpilot-proto-ldp/    ← LDP 协议
  netpilot-proto-pim/    ← PIM 协议
  netpilot-proto-rip/    ← RIP 协议
  netpilot-proto-bfd/    ← BFD 协议
  netpilot-proto-rpki/   ← RPKI 协议
  netpilot-web/          ← React NOC 仪表板
  netpilotd/             ← 守护进程入口
```

## 许可

Apache 2.0
