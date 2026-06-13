#!/bin/bash
set -e
BASE="http://127.0.0.1:8080"
PASS=0
FAIL=0

check() {
    local desc="$1"
    local expected="$2"
    local cmd="$3"
    result=$(eval "$cmd" 2>/dev/null || echo "ERROR")
    if echo "$result" | grep -q "$expected"; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected '$expected', got '$result')"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== NetPilot Smoke Tests ==="

# 1. Health endpoint
check "Health endpoint returns OK" "ok" "curl -s $BASE/health"

# 2. GET running config
check "Running config has schema_version" "schema_version" "curl -s $BASE/api/config/running"

# 3. PUT candidate config (BGP)
BGP_CONFIG='{"schema_version":1,"identity":{"router_id":"192.0.2.1","local_asn":64512},"protocols":[{"kind":"bgp","name":"test-bgp","table":"master","local_asn":64512,"neighbors":[{"name":"peer1","remote_address":"192.0.2.2","remote_asn":64513,"address_families":["ipv4"]}]}]}'
check "PUT candidate config" "204" "curl -s -o /dev/null -w '%{http_code}' -X PUT -H 'Content-Type: application/json' -d '$BGP_CONFIG' $BASE/api/config/candidate"

# 4. Verify candidate
check "GET candidate has BGP" "test-bgp" "curl -s $BASE/api/config/candidate"

# 5. Commit
COMMIT='{"author":"smoke-test","note":"automated smoke test"}'
check "POST commit" "revision" "curl -s -X POST -H 'Content-Type: application/json' -d '$COMMIT' $BASE/api/config/commit"

# 6. Verify running config after commit
check "Running config has committed BGP" "test-bgp" "curl -s $BASE/api/config/running"

# 7. Web UI static files
check "Web UI serves index.html" "NetPilot" "curl -s $BASE/"

# Summary
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ $FAIL -eq 0 ] && exit 0 || exit 1
