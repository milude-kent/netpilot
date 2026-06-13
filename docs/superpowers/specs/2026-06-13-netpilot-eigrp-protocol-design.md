# NetPilot M6 — EIGRP Protocol Design

Date: 2026-06-13

## Goal

Implement EIGRP routing protocol (#304) as the second complete protocol actor. DUAL algorithm, Reliable Transport Protocol (RTP), composite metric, and three-table architecture (neighbor, topology, routing).

## Scope

### In

| Component | Description |
|-----------|-------------|
| EigrpConfig + EigrpInterfaceConfig | Schema + ProtocolConfig variant |
| EigrpActor | ProtocolActor impl with select! loop |
| Neighbor table | Adjacency tracking, hold timer, SRTT, RTO |
| Topology table | All routes from all neighbors, FD/RD |
| DUAL algorithm | Feasible successor calculation, loop-free path selection |
| RTP | Reliable multicast: Update/Query/Reply/ACK, sequence numbers, retransmission |
| Packet types | Hello, Update, Query, Reply, ACK |
| TLV types | EIGRP Parameter, IP Internal/External Route |
| Composite metric | Bandwidth + delay (K1,K3 default), MTU/hop-count/reliability/load (K2,K4,K5) |
| CLI | show eigrp neighbors, show eigrp topology, show eigrp routes |

### Out
- EIGRP authentication (MD5/SHA)
- EIGRP stub routing
- EIGRP named mode
