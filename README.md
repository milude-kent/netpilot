# RoutePlane

RoutePlane is a Rust routing platform inspired by BIRD2. The first milestone builds the daemon foundation, structured configuration model, candidate/running commit workflow, rollback support, and a small REST API.

The long-term architecture is a Rust microkernel with protocol actors for BGP, OSPF, RIP, Babel, Static, Direct, Kernel, BFD, RPKI, MRT, and Pipe.

## First Milestone

- `routeplane-config`: structured config schema, validation, diff, revision history, commit, and rollback.
- `routeplaned`: all-in-one daemon with a REST API for health and configuration workflow.

## Development

```powershell
cargo test
cargo run -p routeplaned
```
