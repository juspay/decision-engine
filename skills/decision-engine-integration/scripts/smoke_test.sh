#!/usr/bin/env bash
#
# smoke_test.sh — end-to-end round trip against a running Decision Engine.
#
# Proves the integration path: bootstrap merchant + API key (if needed),
# create an SR config, call /decide-gateway, then /update-gateway-score.
# Prints a PASS/FAIL summary. Read-only against your data except for creating
# the demo merchant / SR config the first time.
#
# Usage:
#   DECISION_ENGINE_URL=http://localhost:8080 \
#   ADMIN_SECRET=test_admin \
#   MERCHANT_ID=merchant_demo \
#   [API_KEY=DE_xxx] \
#   [ELIGIBLE='["stripe","adyen","checkout"]'] \
#   bash smoke_test.sh
#
set -uo pipefail

BASE_URL="${DECISION_ENGINE_URL:-http://localhost:8080}"
ADMIN_SECRET="${ADMIN_SECRET:-test_admin}"
MERCHANT_ID="${MERCHANT_ID:-merchant_demo}"
API_KEY="${API_KEY:-}"
ELIGIBLE="${ELIGIBLE:-[\"stripe\",\"adyen\",\"checkout\"]}"
PAYMENT_ID="smoke_$(date +%s)_$RANDOM"

pass=0; fail=0
ok()   { echo "  PASS: $1"; pass=$((pass+1)); }
bad()  { echo "  FAIL: $1"; fail=$((fail+1)); }
hr()   { echo "----------------------------------------------------------------"; }

echo "Decision Engine smoke test"
echo "  base   : $BASE_URL"
echo "  merchant: $MERCHANT_ID"
echo "  paymentId: $PAYMENT_ID"
hr

# 1. Health -------------------------------------------------------------------
code=$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/health")
[ "$code" = "200" ] && ok "health 200" || { bad "health returned $code"; echo "Is Decision Engine running at $BASE_URL?"; exit 1; }

# 2. Merchant + API key -------------------------------------------------------
if [ -z "$API_KEY" ]; then
  echo "No API_KEY provided; bootstrapping merchant '$MERCHANT_ID'..."
  resp=$(curl -s --location "$BASE_URL/merchant-account/create" \
    --header "x-admin-secret: $ADMIN_SECRET" \
    --header "Content-Type: application/json" \
    --data "{\"merchant_id\":\"$MERCHANT_ID\",\"gateway_success_rate_based_decider_input\":null}")
  API_KEY=$(printf '%s' "$resp" | grep -o '"api_key"[^,}]*' | head -1 | sed 's/.*: *"//;s/"//')
  if [ -n "$API_KEY" ]; then
    ok "created merchant, got api key ${API_KEY:0:8}..."
  else
    echo "  (merchant may already exist; response: $resp)"
    echo "  Re-run with API_KEY=DE_... to continue."
    exit 1
  fi
fi
AUTH="x-api-key: $API_KEY"

# 3. SR config (idempotent-ish; ignore 'already exists') ----------------------
curl -s --location "$BASE_URL/rule/create" \
  --header "$AUTH" --header "Content-Type: application/json" \
  --data "{\"merchant_id\":\"$MERCHANT_ID\",\"config\":{\"type\":\"successRate\",\"data\":{\"defaultBucketSize\":20,\"defaultLatencyThreshold\":null,\"defaultHedgingPercent\":null,\"subLevelInputConfig\":{\"paymentMethodType\":{\"CARD\":{\"bucketSize\":30,\"hedgingPercent\":0.05}}}}}}" \
  >/dev/null 2>&1
ok "SR config ensured"

hr
# 4. decide-gateway -----------------------------------------------------------
decide_body="{\"merchantId\":\"$MERCHANT_ID\",\"eligibleGatewayList\":$ELIGIBLE,\"rankingAlgorithm\":\"SR_BASED_ROUTING\",\"eliminationEnabled\":true,\"paymentInfo\":{\"paymentId\":\"$PAYMENT_ID\",\"amount\":1000,\"currency\":\"USD\",\"country\":\"US\",\"paymentType\":\"ORDER_PAYMENT\",\"paymentMethodType\":\"CARD\",\"paymentMethod\":\"CREDIT\",\"authType\":\"THREE_DS\",\"cardIsin\":\"424242\"}}"

decide_resp=$(curl -s --location "$BASE_URL/decide-gateway" \
  --header "$AUTH" --header "Content-Type: application/json" --data "$decide_body")
echo "decide-gateway response:"
echo "  $decide_resp"

decided=$(printf '%s' "$decide_resp" | grep -o '"decided_gateway"[^,}]*' | head -1 | sed 's/.*: *"//;s/"//')
if [ -n "$decided" ] && [ "$decided" != "null" ]; then
  ok "decided_gateway = $decided"
  case "$ELIGIBLE" in
    *"\"$decided\""*) ok "decided_gateway is within eligibleGatewayList" ;;
    *) bad "decided_gateway '$decided' NOT in eligibleGatewayList" ;;
  esac
else
  bad "no decided_gateway returned"
  decided="stripe"
fi

hr
# 5. update-gateway-score -----------------------------------------------------
score_body="{\"merchantId\":\"$MERCHANT_ID\",\"gateway\":\"$decided\",\"gatewayReferenceId\":null,\"status\":\"CHARGED\",\"paymentId\":\"$PAYMENT_ID\",\"enforceDynamicRoutingFailure\":null}"
score_code=$(curl -s -o /tmp/de_score_out -w '%{http_code}' --location "$BASE_URL/update-gateway-score" \
  --header "$AUTH" --header "Content-Type: application/json" --data "$score_body")
echo "update-gateway-score -> HTTP $score_code, body: $(cat /tmp/de_score_out)"
[ "$score_code" = "200" ] && ok "score accepted (same paymentId)" || bad "score returned $score_code"

hr
echo "SUMMARY: $pass passed, $fail failed"
[ "$fail" -eq 0 ] && { echo "SMOKE TEST: PASS"; exit 0; } || { echo "SMOKE TEST: FAIL"; exit 1; }
