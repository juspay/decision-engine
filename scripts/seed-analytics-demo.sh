#!/usr/bin/env bash

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8080}"
TENANT_ID="${TENANT_ID:-public}"
MERCHANT_IDS_CSV="${MERCHANT_IDS_CSV:-merchant_space,merchant_nova,merchant_aurora,merchant_orbit,merchant_zephyr,merchant_pulse}"
LIVE_PAYMENTS_PER_CONNECTOR="${LIVE_PAYMENTS_PER_CONNECTOR:-2}"
RESET_SEED_ROWS="${RESET_SEED_ROWS:-true}"
DB_USER="${DB_USER:-db_user}"
DB_PASSWORD="${DB_PASSWORD:-db_pass}"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-decision_engine_db}"
DATABASE_URL="${DATABASE_URL:-postgresql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}}"
SEED_PREFIX="${SEED_PREFIX:-seed_demo}"

IFS=',' read -r -a MERCHANT_IDS <<< "${MERCHANT_IDS_CSV}"
CONNECTOR_PROFILES=(
  "stripe adyen paypal checkout"
  "stripe adyen paypal checkout"
  "stripe adyen paypal braintree"
  "stripe adyen paypal braintree"
  "stripe adyen checkout braintree"
  "stripe paypal checkout braintree"
)
COMMON_CONNECTORS=("stripe" "adyen" "paypal")
EXTRA_CONNECTORS=("checkout" "braintree")
ROUTES=("decision_gateway" "update_gateway_score")
PAYMENT_DIMENSIONS=(
  "CARD CREDIT"
  "CARD DEBIT"
  "WALLET APPLE_PAY"
  "WALLET GOOGLE_PAY"
  "BANK_REDIRECT IDEAL"
  "UPI UPI_COLLECT"
)

header_args=(
  -H "Content-Type: application/json"
  -H "x-tenant-id: ${TENANT_ID}"
)

request() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  local tmp_file
  local code

  tmp_file="$(mktemp)"

  if [[ -n "${body}" ]]; then
    code="$(curl -sS -o "${tmp_file}" -w "%{http_code}" -X "${method}" "${BASE_URL}${path}" "${header_args[@]}" --data "${body}")"
  else
    code="$(curl -sS -o "${tmp_file}" -w "%{http_code}" -X "${method}" "${BASE_URL}${path}" "${header_args[@]}")"
  fi

  if [[ "${code}" -lt 200 || "${code}" -ge 300 ]]; then
    echo "Request failed: ${method} ${path} -> ${code}" >&2
    cat "${tmp_file}" >&2
    rm -f "${tmp_file}"
    return 1
  fi

  cat "${tmp_file}"
  rm -f "${tmp_file}"
}

request_allow_failure() {
  local method="$1"
  local path="$2"
  local body="${3:-}"

  if [[ -n "${body}" ]]; then
    curl -sS -X "${method}" "${BASE_URL}${path}" "${header_args[@]}" --data "${body}" >/dev/null || true
  else
    curl -sS -X "${method}" "${BASE_URL}${path}" "${header_args[@]}" >/dev/null || true
  fi
}

ensure_merchant() {
  local merchant_id="$1"

  if curl -sSf "${BASE_URL}/merchant-account/${merchant_id}" "${header_args[@]}" >/dev/null 2>&1; then
    echo "Merchant ${merchant_id} already exists"
    return
  fi

  echo "Creating merchant ${merchant_id}"
  request "POST" "/merchant-account/create" "{
    \"merchant_id\": \"${merchant_id}\",
    \"gateway_success_rate_based_decider_input\": null
  }" >/dev/null
}

get_profile_for_merchant() {
  local merchant_index="$1"
  local profile_index="$(( merchant_index % ${#CONNECTOR_PROFILES[@]} ))"
  echo "${CONNECTOR_PROFILES[profile_index]}"
}

timestamp_ms_for_slot() {
  local slot="$1"
  python3 - "$slot" <<'PY'
from datetime import datetime, timedelta
import sys

slot = sys.argv[1]
now = datetime.now().astimezone()

if slot == "today_10":
    dt = now.replace(hour=10, minute=0, second=0, microsecond=0)
elif slot == "today_12":
    dt = now.replace(hour=12, minute=0, second=0, microsecond=0)
elif slot == "now":
    dt = now
else:
    raise SystemExit(f"unknown slot: {slot}")

if dt > now and slot != "now":
    dt = dt - timedelta(days=1)

print(int(dt.timestamp() * 1000))
PY
}

status_for_index() {
  local merchant_index="$1"
  local connector_index="$2"
  local slot_index="$3"
  local payment_index="$4"
  local selector="$(( (merchant_index + connector_index + slot_index + payment_index) % 5 ))"

  case "${selector}" in
    0) echo "CHARGED" ;;
    1) echo "FAILURE" ;;
    2) echo "AUTHORIZATION_FAILED" ;;
    3) echo "AUTHENTICATION_FAILED" ;;
    *) echo "CHARGED" ;;
  esac
}

error_code_for_status() {
  local status="$1"
  case "${status}" in
    FAILURE) echo "GATEWAY_TIMEOUT" ;;
    AUTHORIZATION_FAILED) echo "AUTHORIZATION_FAILED" ;;
    AUTHENTICATION_FAILED) echo "AUTHENTICATION_FAILED" ;;
    *) echo "" ;;
  esac
}

error_message_for_status() {
  local status="$1"
  case "${status}" in
    FAILURE) echo "Gateway timeout observed while updating score" ;;
    AUTHORIZATION_FAILED) echo "Authorization failed for the routed gateway" ;;
    AUTHENTICATION_FAILED) echo "Authentication challenge failed for the payment" ;;
    *) echo "" ;;
  esac
}

request_id_for_payment() {
  local payment_id="$1"
  echo "${payment_id}_req"
}

payment_dimension_for_index() {
  local merchant_index="$1"
  local connector_index="$2"
  local slot_index="$3"
  local dimension_index="$(( (merchant_index + connector_index + slot_index) % ${#PAYMENT_DIMENSIONS[@]} ))"
  echo "${PAYMENT_DIMENSIONS[dimension_index]}"
}

seed_live_payment() {
  local merchant_id="$1"
  local gateway="$2"
  local status="$3"
  local payment_id="$4"
  local amount="$5"
  local card_brand="$6"
  local auth_type="$7"

  request "POST" "/decide-gateway" "{
      \"merchantId\": \"${merchant_id}\",
      \"paymentInfo\": {
        \"paymentId\": \"${payment_id}\",
        \"amount\": ${amount},
        \"currency\": \"USD\",
        \"country\": \"US\",
        \"paymentType\": \"ORDER_PAYMENT\",
        \"paymentMethodType\": \"CARD\",
        \"paymentMethod\": \"CREDIT\",
        \"authType\": \"${auth_type}\",
        \"cardBrand\": \"${card_brand}\"
      },
      \"eligibleGatewayList\": [\"${gateway}\"],
      \"rankingAlgorithm\": \"SR_BASED_ROUTING\",
      \"eliminationEnabled\": false
    }" >/dev/null

    request "POST" "/update-gateway-score" "{
      \"merchantId\": \"${merchant_id}\",
      \"gateway\": \"${gateway}\",
      \"gatewayReferenceId\": null,
      \"status\": \"${status}\",
      \"paymentId\": \"${payment_id}\",
      \"enforceDynamicRoutingFailure\": null
    }" >/dev/null
}

seed_live_data() {
  local merchant_id="$1"
  local merchant_index="$2"
  local connectors="$3"
  local connector_index=0

  for gateway in ${connectors}; do
    local payment_index=1
    while (( payment_index <= LIVE_PAYMENTS_PER_CONNECTOR )); do
      local status
      local amount
      local card_brand
      local auth_type
      local payment_id

      status="$(status_for_index "${merchant_index}" "${connector_index}" 2 "${payment_index}")"
      amount="$((1000 + (merchant_index * 190) + (connector_index * 75) + (payment_index * 41)))"
      card_brand="VISA"
      auth_type="THREE_DS"
      if (( (payment_index + connector_index) % 2 == 0 )); then
        card_brand="MASTERCARD"
        auth_type="NO_THREE_DS"
      fi
      payment_id="${SEED_PREFIX}_${merchant_id}_${gateway}_now_${payment_index}_$(date +%s)"

      seed_live_payment "${merchant_id}" "${gateway}" "${status}" "${payment_id}" "${amount}" "${card_brand}" "${auth_type}"
      payment_index=$((payment_index + 1))
    done
    connector_index=$((connector_index + 1))
  done
}

seed_errors() {
  local merchant_id="$1"
  request_allow_failure "POST" "/decide-gateway" '{
    "merchantId":
  }'

  request_allow_failure "POST" "/update-gateway-score" "{
    \"merchantId\": \"${merchant_id}\",
    \"gateway\": \"\",
    \"gatewayReferenceId\": null,
    \"status\": \"CHARGED\",
    \"paymentId\": \"broken_payment\",
    \"enforceDynamicRoutingFailure\": null
  }"
}

cleanup_seed_rows() {
  if [[ "${RESET_SEED_ROWS}" != "true" ]]; then
    return
  fi

  if ! command -v psql >/dev/null 2>&1; then
    return
  fi

  echo "Removing old seeded analytics rows"
  PGPASSWORD="${DB_PASSWORD}" psql "${DATABASE_URL}" >/dev/null <<SQL
DELETE FROM analytics_event
WHERE payment_id LIKE '${SEED_PREFIX}_%'
   OR request_id LIKE '${SEED_PREFIX}_%'
   OR route = 'seed_script';
SQL
}

append_seed_rows_for_payment() {
  local sql_file="$1"
  local merchant_id="$2"
  local gateway="$3"
  local payment_id="$4"
  local request_id="$5"
  local status="$6"
  local base_ms="$7"
  local connector_index="$8"
  local slot_label="$9"
  local score_value="${10}"
  local rule_name="${11}"
  local is_first_row="${12}"
  local payment_method_type="${13}"
  local payment_method="${14}"

  local error_code
  local error_message
  local separator=","
  if [[ "${is_first_row}" == "true" ]]; then
    separator=""
  fi

  error_code="$(error_code_for_status "${status}")"
  error_message="$(error_message_for_status "${status}")"

  cat >> "${sql_file}" <<SQL
${separator}
('decision', '${merchant_id}', '${payment_method_type}', '${payment_method}', '${gateway}', 'gateway_decided', 'SR_BASED_ROUTING', NULL, 'success', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'decision_gateway', '{"slot":"${slot_label}","eligible_gateways":["${gateway}"],"ranking_algorithm":"SR_BASED_ROUTING","seeded":true,"payment_method_type":"${payment_method_type}","payment_method":"${payment_method}"}', ${base_ms}, '${payment_id}', '${request_id}'),
('rule_hit', '${merchant_id}', '${payment_method_type}', '${payment_method}', '${gateway}', 'rule_applied', 'PRIORITY_LOGIC', '${rule_name}', 'hit', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'routing', '{"slot":"${slot_label}","rule_name":"${rule_name}","gateway":"${gateway}","seeded":true,"payment_method_type":"${payment_method_type}","payment_method":"${payment_method}"}', $((base_ms + 30000)), '${payment_id}', '${request_id}'),
('score_snapshot', '${merchant_id}', '${payment_method_type}', '${payment_method}', '${gateway}', 'score_updated', 'SR_BASED_ROUTING', NULL, 'snapshot', NULL, NULL, ${score_value}, $(python3 - <<PY
score = float("${score_value}")
print(round(max(0.08, 1.0 - score), 3))
PY
), $(python3 - <<PY
idx = int("${connector_index}")
print(float(70 + idx * 18))
PY
), $(python3 - <<PY
idx = int("${connector_index}")
print(float(120 + idx * 27))
PY
), $(python3 - <<PY
idx = int("${connector_index}")
print(80 + idx * 11)
PY
), 'update_gateway_score', '{"slot":"${slot_label}","message":"Score snapshot recorded","gateway":"${gateway}","seeded":true,"payment_method_type":"${payment_method_type}","payment_method":"${payment_method}"}', $((base_ms + 60000)), '${payment_id}', '${request_id}')
SQL

  if [[ "${status}" != "CHARGED" ]]; then
    cat >> "${sql_file}" <<SQL
,('error', '${merchant_id}', '${payment_method_type}', '${payment_method}', '${gateway}', 'score_update_failed', 'SR_BASED_ROUTING', NULL, 'failure', '${error_code}', '${error_message}', NULL, NULL, NULL, NULL, NULL, 'update_gateway_score', '{"slot":"${slot_label}","status":"${status}","payment_id":"${payment_id}","seeded":true,"payment_method_type":"${payment_method_type}","payment_method":"${payment_method}"}', $((base_ms + 90000)), '${payment_id}', '${request_id}')
SQL
  fi
}

seed_supporting_analytics_rows() {
  if ! command -v psql >/dev/null 2>&1; then
    echo "psql not found, skipping supplemental analytics rows"
    return
  fi

  local sql_file
  local today_10_ms
  local today_12_ms
  local now_ms
  local first_row="true"
  local merchant_index=0

  sql_file="$(mktemp)"
  today_10_ms="$(timestamp_ms_for_slot today_10)"
  today_12_ms="$(timestamp_ms_for_slot today_12)"
  now_ms="$(timestamp_ms_for_slot now)"

  cat > "${sql_file}" <<SQL
INSERT INTO analytics_event (
  event_type, merchant_id, payment_method_type, payment_method, gateway,
  event_stage, routing_approach, rule_name, status, error_code, error_message, score_value,
  sigma_factor, average_latency, tp99_latency, transaction_count, route, details, created_at_ms,
  payment_id, request_id
) VALUES
SQL

  for merchant_id in "${MERCHANT_IDS[@]}"; do
    local connectors
    local connector_index=0
    connectors="$(get_profile_for_merchant "${merchant_index}")"

    for gateway in ${connectors}; do
      local slot_index=0
      for slot_name in today_10 today_12; do
        local slot_ms
        local status
        local payment_id
        local request_id
        local score_value
        local rule_name
        local payment_method_type
        local payment_method

        if [[ "${slot_name}" == "today_10" ]]; then
          slot_ms="${today_10_ms}"
        else
          slot_ms="${today_12_ms}"
        fi

        status="$(status_for_index "${merchant_index}" "${connector_index}" "${slot_index}" 1)"
        payment_id="${SEED_PREFIX}_${merchant_id}_${gateway}_${slot_name}"
        request_id="$(request_id_for_payment "${payment_id}")"
        read -r payment_method_type payment_method <<< "$(payment_dimension_for_index "${merchant_index}" "${connector_index}" "${slot_index}")"
        score_value="$(python3 - <<PY
merchant_index = int("${merchant_index}")
connector_index = int("${connector_index}")
slot_index = int("${slot_index}")
status = "${status}"
base = 0.965 - (connector_index * 0.04) - (merchant_index % 3) * 0.015 - slot_index * 0.01
if status != "CHARGED":
    base -= 0.08
print(round(max(0.52, min(0.995, base)), 3))
PY
)"
        rule_name="prefer_${gateway}_for_${merchant_id}"

        append_seed_rows_for_payment \
          "${sql_file}" \
          "${merchant_id}" \
          "${gateway}" \
          "${payment_id}" \
          "${request_id}" \
          "${status}" \
          "$((slot_ms + connector_index * 120000 + merchant_index * 300000))" \
          "${connector_index}" \
          "${slot_name}" \
          "${score_value}" \
          "${rule_name}" \
          "${first_row}" \
          "${payment_method_type}" \
          "${payment_method}"

        first_row="false"
        slot_index=$((slot_index + 1))
      done

      local now_payment_id
      local now_request_id
      local now_status
      local now_score_value
      local now_payment_method_type
      local now_payment_method
      now_status="$(status_for_index "${merchant_index}" "${connector_index}" 2 1)"
      now_payment_id="${SEED_PREFIX}_${merchant_id}_${gateway}_now_backfill"
      now_request_id="$(request_id_for_payment "${now_payment_id}")"
      read -r now_payment_method_type now_payment_method <<< "$(payment_dimension_for_index "${merchant_index}" "${connector_index}" 2)"
      now_score_value="$(python3 - <<PY
merchant_index = int("${merchant_index}")
connector_index = int("${connector_index}")
status = "${now_status}"
base = 0.972 - (connector_index * 0.035) - (merchant_index % 4) * 0.012
if status != "CHARGED":
    base -= 0.07
print(round(max(0.55, min(0.996, base)), 3))
PY
)"
      append_seed_rows_for_payment \
        "${sql_file}" \
        "${merchant_id}" \
        "${gateway}" \
        "${now_payment_id}" \
        "${now_request_id}" \
        "${now_status}" \
        "$((now_ms - 600000 + connector_index * 90000 + merchant_index * 240000))" \
        "${connector_index}" \
        "now" \
        "${now_score_value}" \
        "live_${gateway}_followup" \
        "${first_row}" \
        "${now_payment_method_type}" \
        "${now_payment_method}"
      first_row="false"

      connector_index=$((connector_index + 1))
    done

    merchant_index=$((merchant_index + 1))
  done

  echo ";" >> "${sql_file}"
  PGPASSWORD="${DB_PASSWORD}" psql "${DATABASE_URL}" >/dev/null -f "${sql_file}"
  rm -f "${sql_file}"
}

main() {
  echo "Seeding analytics demo data into ${BASE_URL}"

  cleanup_seed_rows

  local merchant_index=0
  for merchant_id in "${MERCHANT_IDS[@]}"; do
    local connectors
    connectors="$(get_profile_for_merchant "${merchant_index}")"

    ensure_merchant "${merchant_id}"

    echo "Generating live decision and score events for ${merchant_id} using: ${connectors}"
    seed_live_data "${merchant_id}" "${merchant_index}" "${connectors}"

    if (( merchant_index < 2 )); then
      echo "Generating a few structured API errors for ${merchant_id}"
      seed_errors "${merchant_id}"
    fi

    merchant_index=$((merchant_index + 1))
  done

  echo "Adding supplemental score snapshots, rule hits, and timeline backfill rows"
  seed_supporting_analytics_rows

  cat <<EOF

Done.

Merchants created or reused:
  ${MERCHANT_IDS_CSV}

Analytics data should now appear under:
  /dashboard/analytics
  /dashboard/audit

EOF
}

main "$@"
