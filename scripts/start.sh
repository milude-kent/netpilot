#!/bin/bash
set -e

echo "=== NetPilot Routing Platform ==="

# Build if needed
if [ ! -f target/debug/netpilotd ]; then
    echo "Building..."
    cargo build -p netpilotd
fi

# Build web UI if needed
if [ ! -d crates/netpilot-web/dist ]; then
    echo "Building Web UI..."
    cd crates/netpilot-web && npm install && npm run build && cd ../..
fi

echo "Starting NetPilot daemon..."
echo "  REST API : http://127.0.0.1:8080"
echo "  Web UI   : http://127.0.0.1:8080/"
echo "  gRPC     : http://127.0.0.1:50051"
echo ""

cargo run -p netpilotd
