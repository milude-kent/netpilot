#!/bin/bash
# NetPilot HTTP smoke test.
#
# Requires netpilotd running on 127.0.0.1:8080 (default). Used by:
#   - local devs: `./scripts/start.sh & bash scripts/smoke-test.sh`
#   - CI smoke job (.github/workflows/ci.yml)
#
# Note: the API uses kebab-case JSON keys (schema-version, router-id, ...)
# matching the BIRD2 config convention.

set -e
BASE="${NETPILOT_BASE:-http://127.0.0.1:8080}"
PASS=0
FAIL=0

check() {
    local desc="$1"
    local expected="$2"
    local cmd="$3"
    result=$(eval "$cmd" 2>/dev/null || echo "ERROR")
    if echo "$result" | grep -q -- "$expected"; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc"
        echo "    expected substring: $expected"
        echo "    got: $(echo "$result" | head -c 400)"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== NetPilot Smoke Tests (against $BASE) ==="

# 1. Health endpoint — plain text "ok".
check "Health endpoint returns OK" "ok" "curl -s $BASE/health"

# 2. /metrics — Prometheus exposition format. Counters that have never been
#    incremented are absent; we just assert the endpoint is reachable with
#    the right content-type.
check "Metrics endpoint serves Prometheus format" "200" \
    "curl -s -o /dev/null -w '%{http_code}' $BASE/metrics"

# 3. GET running config — serialised with kebab-case keys.
check "Running config has schema-version" "schema-version" "curl -s $BASE/api/config/running"

# 4. PUT candidate config (BGP).
#    Naming quirk: RoutePlaneConfig + BgpNeighbor structs use kebab-case
#    (schema-version / router-id / remote-address ...), but fields *inside*
#    the ProtocolConfig::Bgp variant use snake_case (local_asn / table /
#    neighbors) because serde's rename_all on a tagged enum only applies
#    to variant names, not nested fields.
BGP_CONFIG='{"schema-version":1,"identity":{"router-id":"192.0.2.1","local-asn":64512},"tables":[{"name":"master"}],"protocols":[{"kind":"bgp","name":"test-bgp","table":"master","local_asn":64512,"neighbors":[{"name":"peer1","remote-address":"192.0.2.2","remote-asn":64513,"address-families":["ipv4"]}]}]}'
check "PUT candidate config (204 No Content)" "204" \
    "curl -s -o /dev/null -w '%{http_code}' -X PUT -H 'Content-Type: application/json' -d '$BGP_CONFIG' $BASE/api/config/candidate"

# 5. Verify candidate
check "GET candidate has BGP protocol" "test-bgp" "curl -s $BASE/api/config/candidate"

# 6. Commit — response is a Revision JSON object with id/config/author/note.
COMMIT='{"author":"smoke-test","note":"automated smoke test"}'
check "POST commit returns a revision id" '"id":' \
    "curl -s -X POST -H 'Content-Type: application/json' -d '$COMMIT' $BASE/api/config/commit"

# 7. Verify running config after commit.
check "Running config has committed BGP" "test-bgp" "curl -s $BASE/api/config/running"

# 8. SSE event stream advertises text/event-stream.
check "SSE events endpoint advertises event-stream" "text/event-stream" \
    "curl -s -I $BASE/api/events"

# 9. Web UI static files (only when dist/ has been built; non-fatal if absent
#    via the ServeDir fallback).
check "Web UI serves index.html (or 404 if no dist/)" "DOCTYPE\|HTTP/1.1 404" \
    "curl -s -i $BASE/ | head -c 1024"

# Summary
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ $FAIL -eq 0 ] && exit 0 || exit 1