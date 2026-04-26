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

# ── 8. User signup (no merchant_id required) ───────────
echo ""
echo "[ User signup ]"
SIGNUP_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/signup" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"$TEST_PASSWORD\"}")

JWT_TOKEN=$(echo "$SIGNUP_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
SIGNUP_MERCHANT_ID=$(echo "$SIGNUP_RESPONSE" | grep -o '"merchant_id":"[^"]*"' | cut -d'"' -f4)
SIGNUP_MERCHANTS=$(echo "$SIGNUP_RESPONSE" | grep -o '"merchants":\[\]')
if [ -n "$JWT_TOKEN" ]; then
    echo "  PASS  POST /auth/signup returns JWT token"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /auth/signup — unexpected response: $SIGNUP_RESPONSE"
    FAIL=$((FAIL + 1))
fi
if [ -z "$SIGNUP_MERCHANT_ID" ] || [ "$SIGNUP_MERCHANT_ID" = '""' ]; then
    echo "  PASS  POST /auth/signup — merchant_id is empty (onboarding pending)"
    PASS=$((PASS + 1))
else
    echo "  INFO  POST /auth/signup — merchant_id: $SIGNUP_MERCHANT_ID"
fi
if [ -n "$SIGNUP_MERCHANTS" ]; then
    echo "  PASS  POST /auth/signup — merchants list is empty"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /auth/signup — expected empty merchants list, got: $SIGNUP_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 9. Duplicate signup → 409 ─────────────────────────
echo ""
echo "[ Duplicate signup ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/auth/signup" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"$TEST_PASSWORD\"}")
check "Duplicate email → 409" "409" "$STATUS"

# ── 10. Onboarding: create first merchant ──────────────
echo ""
echo "[ Onboarding: create merchant ]"
ONBOARD_RESPONSE=$(curl -s -X POST "$BASE_URL/onboarding/merchant" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $JWT_TOKEN" \
    -d "{\"merchant_name\": \"Test Corp\"}")

ONBOARD_TOKEN=$(echo "$ONBOARD_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
ONBOARD_MID=$(echo "$ONBOARD_RESPONSE" | grep -o '"merchant_id":"[^"]*"' | head -1 | cut -d'"' -f4)
ONBOARD_NAME=$(echo "$ONBOARD_RESPONSE" | grep -o '"merchant_name":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -n "$ONBOARD_TOKEN" ] && [ -n "$ONBOARD_MID" ]; then
    echo "  PASS  POST /onboarding/merchant returns token + merchant_id"
    echo "        merchant_id: $ONBOARD_MID"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /onboarding/merchant — unexpected response: $ONBOARD_RESPONSE"
    FAIL=$((FAIL + 1))
fi
check "Merchant name stored correctly" "Test Corp" "$ONBOARD_NAME"

ONBOARD_COUNT=$(echo "$ONBOARD_RESPONSE" | grep -o '"merchant_id"' | wc -l | tr -d ' ')
if [ "$ONBOARD_COUNT" -ge "1" ]; then
    echo "  PASS  POST /onboarding/merchant — merchants list has 1 entry"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /onboarding/merchant — merchants list empty: $ONBOARD_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 11. Onboarding: create second merchant ─────────────
echo ""
echo "[ Onboarding: create second merchant ]"
ONBOARD2_RESPONSE=$(curl -s -X POST "$BASE_URL/onboarding/merchant" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $ONBOARD_TOKEN" \
    -d "{\"merchant_name\": \"Beta Inc\"}")

ONBOARD2_MID=$(echo "$ONBOARD2_RESPONSE" | grep -o '"merchant_id":"[^"]*"' | head -1 | cut -d'"' -f4)
ONBOARD2_COUNT=$(echo "$ONBOARD2_RESPONSE" | grep -o '"merchant_id"' | wc -l | tr -d ' ')
ONBOARD2_TOKEN=$(echo "$ONBOARD2_RESPONSE" | grep -o '"token":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -n "$ONBOARD2_MID" ]; then
    echo "  PASS  POST /onboarding/merchant — second merchant created"
    echo "        merchant_id: $ONBOARD2_MID"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /onboarding/merchant (2nd) — unexpected: $ONBOARD2_RESPONSE"
    FAIL=$((FAIL + 1))
fi
if [ "$ONBOARD2_COUNT" -ge "2" ]; then
    echo "  PASS  merchants list now has $ONBOARD2_COUNT entries"
    PASS=$((PASS + 1))
else
    echo "  FAIL  Expected 2+ merchants in list, got $ONBOARD2_COUNT: $ONBOARD2_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 12. List merchants ─────────────────────────────────
echo ""
echo "[ List merchants ]"
LIST_MERCHANTS=$(curl -s "$BASE_URL/auth/merchants" \
    -H "Authorization: Bearer $ONBOARD2_TOKEN")
LIST_COUNT=$(echo "$LIST_MERCHANTS" | grep -o '"merchant_id"' | wc -l | tr -d ' ')
if [ "$LIST_COUNT" -ge "2" ]; then
    echo "  PASS  GET /auth/merchants returns $LIST_COUNT merchants"
    PASS=$((PASS + 1))
else
    echo "  FAIL  GET /auth/merchants — got: $LIST_MERCHANTS"
    FAIL=$((FAIL + 1))
fi

# ── 13. Switch merchant ────────────────────────────────
echo ""
echo "[ Switch merchant ]"
SWITCH_RESPONSE=$(curl -s --max-time 10 -X POST "$BASE_URL/auth/switch-merchant" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $ONBOARD2_TOKEN" \
    -d "{\"merchant_id\": \"$ONBOARD_MID\"}")

SWITCH_TOKEN=$(echo "$SWITCH_RESPONSE" | grep -o '"token":"[^"]*"' | head -1 | cut -d'"' -f4)
SWITCH_MID=$(echo "$SWITCH_RESPONSE" | grep -o '"merchant_id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -n "$SWITCH_TOKEN" ]; then
    echo "  PASS  POST /auth/switch-merchant returns new token"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /auth/switch-merchant — unexpected: $SWITCH_RESPONSE"
    FAIL=$((FAIL + 1))
fi
check "Switch sets correct active merchant_id" "$ONBOARD_MID" "$SWITCH_MID"

echo ""
echo "[ Switch to non-existent merchant → 404 ]"
STATUS=$(curl -s --max-time 10 -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/auth/switch-merchant" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $ONBOARD2_TOKEN" \
    -d '{"merchant_id": "merchant_doesnotexist"}')
check "Switch to unknown merchant → 404" "404" "$STATUS"

# ── 14. Login returns merchants list ──────────────────
echo ""
echo "[ User login ]"
LOGIN_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"$TEST_PASSWORD\"}")

JWT_TOKEN=$(echo "$LOGIN_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
LOGIN_MERCHANT_COUNT=$(echo "$LOGIN_RESPONSE" | grep -o '"merchant_id"' | wc -l | tr -d ' ')
if [ -n "$JWT_TOKEN" ]; then
    echo "  PASS  POST /auth/login returns JWT token"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /auth/login — unexpected response: $LOGIN_RESPONSE"
    FAIL=$((FAIL + 1))
fi
if [ "$LOGIN_MERCHANT_COUNT" -ge "2" ]; then
    echo "  PASS  POST /auth/login — merchants list populated ($LOGIN_MERCHANT_COUNT entries)"
    PASS=$((PASS + 1))
else
    echo "  FAIL  POST /auth/login — merchants list missing or empty: $LOGIN_RESPONSE"
    FAIL=$((FAIL + 1))
fi

# ── 15. Wrong password → 401 ──────────────────────────
echo ""
echo "[ Wrong password ]"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"email\": \"$TEST_EMAIL\", \"password\": \"wrongpassword\"}")
check "Wrong password → 401" "401" "$STATUS"

# ── 16. JWT accesses protected route ──────────────────
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

    # ── 17. /auth/me ──────────────────────────────────
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
    ME_MERCHANT_COUNT=$(echo "$ME_RESPONSE" | grep -o '"merchant_id"' | wc -l | tr -d ' ')
    if [ "$ME_MERCHANT_COUNT" -ge "2" ]; then
        echo "  PASS  GET /auth/me — merchants list populated"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  GET /auth/me — merchants list missing: $ME_RESPONSE"
        FAIL=$((FAIL + 1))
    fi

    # ── 18. Logout ────────────────────────────────────
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

    # ── 19. Revoked JWT rejected ──────────────────────
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

    # ── 20. Re-login after logout ─────────────────────
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

# ── 21. Redis cache hit (API key) ──────────────────────
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
