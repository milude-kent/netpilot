# NetPilot

NetPilot is a Rust routing platform inspired by BIRD2. The first milestone builds the daemon foundation, structured configuration model, candidate/running commit workflow, rollback support, and a small REST API.

The long-term architecture is a Rust microkernel with protocol actors for BGP, OSPF, RIP, Babel, Static, Direct, Kernel, BFD, RPKI, MRT, and Pipe.

## First Milestone

- `netpilot-config`: structured config schema, validation, diff, revision history, commit, and rollback.
- `netpilotd`: all-in-one daemon with a REST API for health and configuration workflow.

## Development

```powershell
cargo test
cargo run -p netpilotd
```

## Verification

```powershell
cargo fmt --check
cargo test
cargo run -p netpilotd
```

The API listens on `127.0.0.1:8080` in the first milestone.
