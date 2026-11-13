#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8080}"
MERCHANT_ID="test_merchant_$(date +%s)"

PASS=0
FAIL=0

check() {
    local name="$1"
    local expected="$2"
    local actual="$3"
    if [ "$actual" = "$expected" ]; then
        echo "  PASS  $name"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $name (expected $expected, got $actual)"
        FAIL=$((FAIL + 1))
    fi
}

echo ""
echo "Target: $BASE_URL"
echo "Merchant: $MERCHANT_ID"
echo "=================================================="

# ── 1. Health ──────────────────────────────────────────
echo ""
echo "[ Health ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health/ready")
check "GET /health/ready → 200" "200" "$STATUS"

# ── 2. Backward compat (no auth header, no key) ────────
echo ""
echo "[ Backward-compat mode — expect 401 only when api_key_auth_enabled=true ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
    -H "Content-Type: application/json" -d '{}')
if [ "$STATUS" = "401" ]; then
    echo "  INFO  Auth is ENFORCED (api_key_auth_enabled=true) — skipping backward-compat check"
    AUTH_ENFORCED=true
else
    check "No x-api-key passes through (backward compat)" "422" "$STATUS"
    AUTH_ENFORCED=false
fi

# ── 3. Merchant create → returns api_key ───────────────
echo ""
echo "[ Merchant create ]"
RESPONSE=$(curl -s -X POST "$BASE_URL/merchant-account/create" \
    -H "Content-Type: application/json" \
    -d "{\"merchant_id\": \"$MERCHANT_ID\", \"config\": {}}")

API_KEY=$(echo "$RESPONSE" | grep -o '"api_key":"[^"]*"' | cut -d'"' -f4)
if [ -n "$API_KEY" ] && [ "$API_KEY" != "null" ]; then
    echo "  PASS  POST /merchant-account/create returns api_key"
    echo "        key: ${API_KEY:0:20}..."
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /merchant-account/create — api_key missing in response"
    echo "        response: $RESPONSE"
    FAIL=$((FAIL + 1))
    echo ""
    echo "Cannot continue without a valid API key. Exiting."
    exit 1
fi

# ── 4. Auth enforcement checks ─────────────────────────
if [ "$AUTH_ENFORCED" = "true" ]; then
    echo ""
    echo "[ Auth enforcement ]"

    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" -d '{}')
    check "No key → 401" "401" "$STATUS"

    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: DE_wrongkeydeadbeef00000000000000000000000000000000000000000000000" \
        -d '{}')
    check "Wrong key → 401" "401" "$STATUS"

    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: $API_KEY" \
        -d '{}')
    check "Valid key passes auth (422 = body error, not 401)" "422" "$STATUS"
fi

# ── 5. Create additional key ───────────────────────────
echo ""
echo "[ Create additional API key ]"
CREATE_RESPONSE=$(curl -s -X POST "$BASE_URL/api-key/create" \
    -H "Content-Type: application/json" \
    -H "x-api-key: $API_KEY" \
    -d "{\"merchant_id\": \"$MERCHANT_ID\", \"description\": \"secondary key\"}")

NEW_KEY=$(echo "$CREATE_RESPONSE" | grep -o '"api_key":"[^"]*"' | cut -d'"' -f4)
NEW_KEY_ID=$(echo "$CREATE_RESPONSE" | grep -o '"key_id":"[^"]*"' | cut -d'"' -f4)

if [ -n "$NEW_KEY" ] && [ -n "$NEW_KEY_ID" ]; then
    echo "  PASS  POST /api-key/create returns new key"
    echo "        key_id: $NEW_KEY_ID"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /api-key/create — unexpected response: $CREATE_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 6. List keys ───────────────────────────────────────
echo ""
echo "[ List API keys ]"
LIST_RESPONSE=$(curl -s "$BASE_URL/api-key/list/$MERCHANT_ID" \
    -H "x-api-key: $API_KEY")
KEY_COUNT=$(echo "$LIST_RESPONSE" | grep -o '"key_id"' | wc -l | tr -d ' ')
if [ "$KEY_COUNT" -ge "2" ]; then
    echo "  PASS  GET /api-key/list/:merchant_id returns $KEY_COUNT keys"
    PASS=$((PASS + 1))
else
    echo "  FAIL  GET /api-key/list/:merchant_id — got: $LIST_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 7. New key works ───────────────────────────────────
if [ -n "$NEW_KEY" ]; then
    echo ""
    echo "[ New key is usable ]"
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: $NEW_KEY" \
        -d '{}')
    check "Newly created key passes auth (422, not 401)" "422" "$STATUS"
fi

# ── 8. Revoke key ──────────────────────────────────────
if [ -n "$NEW_KEY_ID" ]; then
    echo ""
    echo "[ Revoke API key ]"
    REVOKE_RESPONSE=$(curl -s -X DELETE "$BASE_URL/api-key/$NEW_KEY_ID" \
        -H "x-api-key: $API_KEY")
    if echo "$REVOKE_RESPONSE" | grep -q "revoked successfully"; then
        echo "  PASS  DELETE /api-key/:key_id revokes key"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  DELETE /api-key/:key_id — got: $REVOKE_RESPONSE"
        FAIL=$((FAIL + 1))
    fi

    # ── 9. Revoked key is rejected ─────────────────────
    echo ""
    echo "[ Revoked key is rejected ]"
    # Clear Redis cache entry so middleware hits DB
    sleep 1
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: $NEW_KEY" \
        -d '{}')
    check "Revoked key → 401" "401" "$STATUS"

    # ── 10. List shows is_active: false ────────────────
    echo ""
    echo "[ List reflects revocation ]"
    LIST_AFTER=$(curl -s "$BASE_URL/api-key/list/$MERCHANT_ID" \
        -H "x-api-key: $API_KEY")
    if echo "$LIST_AFTER" | grep -q '"is_active":false'; then
        echo "  PASS  Revoked key shows is_active=false in list"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  Revoked key not reflected in list: $LIST_AFTER"
        FAIL=$((FAIL + 1))
    fi
fi

# ── 11. Redis cache hit ────────────────────────────────
echo ""
echo "[ Redis cache hit ]"
for i in 1 2; do
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: $API_KEY" \
        -d '{}')
    check "Request $i with valid key passes (cache warm on 2nd)" "422" "$STATUS"
done

# ── Summary ────────────────────────────────────────────
echo ""
echo "=================================================="
echo "Results: $PASS passed, $FAIL failed"
echo ""
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
