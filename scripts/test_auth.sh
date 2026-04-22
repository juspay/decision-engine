#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8080}"
ADMIN_SECRET="${ADMIN_SECRET:-test_admin}"
MERCHANT_ID="test_merchant_$(date +%s)"
TEST_EMAIL="testuser_$(date +%s)@example.com"
TEST_PASSWORD="TestPass@123"

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

# ── 2. Merchant create ─────────────────────────────────
echo ""
echo "[ Merchant create ]"
RESPONSE=$(curl -s -X POST "$BASE_URL/merchant-account/create" \
    -H "Content-Type: application/json" \
    -H "x-admin-secret: $ADMIN_SECRET" \
    -d "{\"merchant_id\": \"$MERCHANT_ID\", \"config\": {}}")

API_KEY=$(echo "$RESPONSE" | grep -o '"api_key":"[^"]*"' | cut -d'"' -f4)
if [ -n "$API_KEY" ] && [ "$API_KEY" != "null" ]; then
    echo "  PASS  POST /merchant-account/create returns api_key"
    echo "        key: ${API_KEY:0:20}..."
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /merchant-account/create — api_key missing"
    echo "        response: $RESPONSE"
    FAIL=$((FAIL + 1))
    echo "Cannot continue without a valid API key. Exiting."
    exit 1
fi

# ── 3. Merchant create without admin secret → 401 ─────
echo ""
echo "[ Admin secret enforcement ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/merchant-account/create" \
    -H "Content-Type: application/json" \
    -d "{\"merchant_id\": \"${MERCHANT_ID}_x\", \"config\": {}}")
check "POST /merchant-account/create without x-admin-secret → 401" "401" "$STATUS"

# ── 4. Auth enforcement checks (API key) ───────────────
echo ""
echo "[ API key auth enforcement ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
    -H "Content-Type: application/json" -d '{}')
if [ "$STATUS" = "401" ]; then
    AUTH_ENFORCED=true
    check "No credentials → 401" "401" "$STATUS"

    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: DE_wrongkeydeadbeef00000000000000000000000000000000000000000000000" \
        -d '{}')
    check "Wrong API key → 401" "401" "$STATUS"

    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: $API_KEY" \
        -d '{}')
    check "Valid API key passes auth (422 = body error, not 401)" "422" "$STATUS"
else
    AUTH_ENFORCED=false
    echo "  INFO  api_key_auth_enabled=false — auth rejection checks skipped"
fi

# ── 5. Create additional API key ───────────────────────
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

# ── 6. List API keys ───────────────────────────────────
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

# ── 7. Revoke secondary key ────────────────────────────
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

    echo ""
    echo "[ Revoked API key is rejected ]"
    sleep 1
    if [ "$AUTH_ENFORCED" = "true" ]; then
        STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
            -H "Content-Type: application/json" \
            -H "x-api-key: $NEW_KEY" \
            -d '{}')
        check "Revoked API key → 401" "401" "$STATUS"
    else
        echo "  INFO  Auth not enforced — skipping revoked key rejection check"
    fi

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

# ── 8. User signup ─────────────────────────────────────
echo ""
echo "[ User signup ]"
SIGNUP_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/signup" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"$TEST_PASSWORD\", \"merchant_id\": \"$MERCHANT_ID\"}")

JWT_TOKEN=$(echo "$SIGNUP_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
if [ -n "$JWT_TOKEN" ]; then
    echo "  PASS  POST /auth/signup returns JWT token"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /auth/signup — unexpected response: $SIGNUP_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 9. Duplicate signup → 409 ─────────────────────────
echo ""
echo "[ Duplicate signup ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/auth/signup" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"$TEST_PASSWORD\", \"merchant_id\": \"$MERCHANT_ID\"}")
check "Duplicate email → 409" "409" "$STATUS"

# ── 10. Login ──────────────────────────────────────────
echo ""
echo "[ User login ]"
LOGIN_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"$TEST_PASSWORD\"}")

JWT_TOKEN=$(echo "$LOGIN_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
if [ -n "$JWT_TOKEN" ]; then
    echo "  PASS  POST /auth/login returns JWT token"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /auth/login — unexpected response: $LOGIN_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 11. Wrong password → 401 ──────────────────────────
echo ""
echo "[ Wrong password ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"wrongpassword\"}")
check "Wrong password → 401" "401" "$STATUS"

# ── 12. JWT accesses protected route ──────────────────
if [ -n "$JWT_TOKEN" ]; then
    echo ""
    echo "[ JWT auth on protected routes ]"
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $JWT_TOKEN" \
        -d '{}')
    check "Valid JWT passes auth (422 = body error, not 401)" "422" "$STATUS"

    if [ "$AUTH_ENFORCED" = "true" ]; then
        STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer invalidtoken.abc.def" \
            -d '{}')
        check "Invalid JWT → 401" "401" "$STATUS"
    else
        echo "  INFO  Auth not enforced — skipping invalid JWT check"
    fi

    # ── 13. /auth/me ──────────────────────────────────
    echo ""
    echo "[ /auth/me ]"
    ME_RESPONSE=$(curl -s "$BASE_URL/auth/me" \
        -H "Authorization: Bearer $JWT_TOKEN")
    if echo "$ME_RESPONSE" | grep -q "\"email\":\"$TEST_EMAIL\""; then
        echo "  PASS  GET /auth/me returns correct user"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  GET /auth/me — got: $ME_RESPONSE"
        FAIL=$((FAIL + 1))
    fi

    # ── 14. Logout ────────────────────────────────────
    echo ""
    echo "[ Logout ]"
    LOGOUT_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/logout" \
        -H "Authorization: Bearer $JWT_TOKEN")
    if echo "$LOGOUT_RESPONSE" | grep -q "Logged out"; then
        echo "  PASS  POST /auth/logout succeeds"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  POST /auth/logout — got: $LOGOUT_RESPONSE"
        FAIL=$((FAIL + 1))
    fi

    # ── 15. Revoked JWT rejected ──────────────────────
    echo ""
    echo "[ Revoked JWT rejected ]"
    sleep 1
    if [ "$AUTH_ENFORCED" = "true" ]; then
        STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $JWT_TOKEN" \
            -d '{}')
        check "Logged-out JWT → 401" "401" "$STATUS"
    else
        echo "  INFO  Auth not enforced — skipping revoked JWT check"
    fi

    # ── 16. Re-login after logout ─────────────────────
    echo ""
    echo "[ Re-login after logout ]"
    RELOGIN_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"$TEST_PASSWORD\"}")
    NEW_JWT=$(echo "$RELOGIN_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
    if [ -n "$NEW_JWT" ]; then
        echo "  PASS  Re-login after logout returns new JWT"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  Re-login after logout failed: $RELOGIN_RESPONSE"
        FAIL=$((FAIL + 1))
    fi
fi

# ── 17. Redis cache hit (API key) ──────────────────────
echo ""
echo "[ Redis cache hit ]"
for i in 1 2; do
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/rule/get" \
        -H "Content-Type: application/json" \
        -H "x-api-key: $API_KEY" \
        -d '{}')
    check "Request $i with valid API key (cache warm on 2nd)" "422" "$STATUS"
done

# ── Summary ────────────────────────────────────────────
echo ""
echo "=================================================="
echo "Results: $PASS passed, $FAIL failed"
echo ""
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
