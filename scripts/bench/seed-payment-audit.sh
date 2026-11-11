#!/usr/bin/env bash
# Seeds a benchmark merchant's payment audit trail straight into ClickHouse: evenly spread payments
# over DAYS days ending at END_MS, each with the events the live flows record, in their stored shapes
# and sizes. The summary tables the audit list reads are filled with the same SELECT their Kafka
# materialized views run.
#
# Each payment is routed by one endpoint:
#   /decide 40% and /routing/hybrid 20%: request hit + routing_hybrid_decision (~5 KB of details)
#   /decide-gateway 30%:                 request hit + decide_gateway_decision (~1.8 KB)
#   /routing/evaluate 10%:               request hit + routing_evaluate_advanced (~0.8 KB)
# and then reported: update_gateway_score request hit, update (~0.5 KB) and score snapshot. Card
# BINs (skewed: a few BINs carry most payments), countries, currencies and a `segment` metadata
# key vary per payment.
#
#   MERCHANT_ID=bench_audit DAYS=30 PAYMENTS_PER_DAY=20000 scripts/bench/seed-payment-audit.sh
#
# Re-running for the same merchant deletes its earlier seed first.
set -euo pipefail

MERCHANT_ID=${MERCHANT_ID:-bench_audit}
DAYS=${DAYS:-30}
PAYMENTS_PER_DAY=${PAYMENTS_PER_DAY:-20000}
END_MS=${END_MS:-$(($(date +%s) * 1000))}
CLICKHOUSE=${CLICKHOUSE:-"docker exec -i decision-engine-clickhouse-1 clickhouse-client"}

payments=$((DAYS * PAYMENTS_PER_DAY))
span_ms=$((DAYS * 86400000))
start_ms=$((END_MS - span_ms))

ch() { $CLICKHOUSE --multiquery "$@"; }

echo "Seeding ${payments} payments for ${MERCHANT_ID} over ${DAYS} days ending at ${END_MS}..."
ch <<SQL
SET mutations_sync = 2;
DELETE FROM analytics_domain_events WHERE merchant_id = '${MERCHANT_ID}';
DELETE FROM analytics_payment_audit_summary_buckets WHERE merchant_id = '${MERCHANT_ID}';
DELETE FROM analytics_payment_audit_lookup_summaries WHERE merchant_id = '${MERCHANT_ID}';

INSERT INTO analytics_domain_events (
    event_id, api_flow, flow_type, summary_kind, merchant_id, payment_id, request_id, lookup_key,
    payment_method_type, payment_method, card_network, currency, gateway, event_stage,
    routing_approach, status, route, details, created_at_ms)
WITH
    ['US', 'GB', 'IN', 'DE', 'FR', 'BR', 'AE', 'SG', 'AU', 'NL'] AS countries,
    ['UnitedStatesOfAmerica', 'UnitedKingdomOfGreatBritainAndNorthernIreland', 'India', 'Germany',
     'France', 'Brazil', 'UnitedArabEmirates', 'Singapore', 'Australia', 'Netherlands'] AS country_names,
    ['USD', 'GBP', 'INR', 'EUR', 'EUR', 'BRL', 'AED', 'SGD', 'AUD', 'EUR'] AS currencies,
    ['premium', 'standard', 'basic', 'enterprise'] AS segments,
    ['stripe', 'adyen', 'checkout', 'worldpay'] AS gateways
SELECT
    concat('${MERCHANT_ID}_', toString(number), '_', toString(kind)) AS event_id,
    if(endpoint = 'evaluate' AND kind < 2, 'rule_based_routing', 'dynamic_routing') AS api_flow,
    multiIf(
        kind = 0, multiIf(endpoint IN ('decide', 'hybrid'), 'routing_hybrid_request_hit',
                          endpoint = 'gateway', 'decide_gateway_request_hit', 'routing_evaluate_request_hit'),
        kind = 1, multiIf(endpoint IN ('decide', 'hybrid'), 'routing_hybrid_decision',
                          endpoint = 'gateway', 'decide_gateway_decision', 'routing_evaluate_advanced'),
        kind = 2, 'update_gateway_score_request_hit',
        kind = 3, 'update_gateway_score_update',
        'update_gateway_score_score_snapshot') AS flow_type,
    multiIf(kind = 1, multiIf(endpoint IN ('decide', 'hybrid'), 'hybrid', endpoint = 'gateway', 'dynamic', 'preview'),
            kind = 3, 'dynamic', NULL) AS summary_kind,
    '${MERCHANT_ID}' AS merchant_id,
    payment_id,
    concat('req_', toString(number), '_', toString(kind)) AS request_id,
    payment_id AS lookup_key,
    if(kind = 1 AND endpoint != 'evaluate', 'CREDIT', NULL) AS payment_method_type,
    if(kind = 1 AND endpoint != 'evaluate', 'CARD', NULL) AS payment_method,
    if(kind = 1 AND endpoint != 'evaluate', 'VISA', NULL) AS card_network,
    if(kind = 1 AND endpoint != 'evaluate', currency, NULL) AS currency,
    if(kind IN (1, 3, 4), gateway, NULL) AS gateway,
    multiIf(kind IN (0, 2), 'request_received',
            kind = 1, multiIf(endpoint = 'decide', 'decide_routed', endpoint = 'hybrid', 'hybrid_routed',
                              endpoint = 'gateway', 'gateway_decided', 'preview_evaluated'),
            'score_updated') AS event_stage,
    if(kind = 1, multiIf(endpoint = 'gateway', 'SR_SELECTION_V3_ROUTING', endpoint = 'evaluate',
                         'RULE_EVALUATE_PREVIEW', 'SR_SELECTION_V3_ROUTING'), NULL) AS routing_approach,
    multiIf(kind IN (0, 2), 'received', kind = 1, 'success', kind = 3, 'CHARGED', 'snapshot') AS status,
    multiIf(kind >= 2, 'update_gateway_score', endpoint = 'decide', 'decide', endpoint = 'hybrid',
            'routing_hybrid', endpoint = 'gateway', 'decide_gateway', 'routing_evaluate') AS route,
    multiIf(
        kind IN (0, 2), NULL,
        kind = 1 AND endpoint IN ('decide', 'hybrid'), concat(
            '{"request":{"static_routing_request":{"payment_id":"', payment_id, '","created_by":"${MERCHANT_ID}",',
            '"fallback_output":[{"gateway_name":"stripe","gateway_id":"mca_stripe"},{"gateway_name":"adyen","gateway_id":"mca_adyen"},{"gateway_name":"checkout","gateway_id":"mca_checkout"},{"gateway_name":"worldpay","gateway_id":"mca_worldpay"}],',
            '"parameters":{"payment_method":{"type":"enum_variant","value":"card"},"payment_method_type":{"type":"enum_variant","value":"credit"},',
            '"card_bin":{"type":"str_value","value":"', bin, '"},"card_network":{"type":"enum_variant","value":"Visa"},',
            '"billing_country":{"type":"enum_variant","value":"', country_name, '"},"currency":{"type":"enum_variant","value":"', currency, '"},',
            '"amount":{"type":"number","value":', toString(amount), '},"payment_amount":{"type":"number","value":', toString(amount), '},',
            '"segment":{"type":"metadata_variant","value":{"key":"segment","value":"', segment, '"}}}},',
            '"dynamic_routing_request":{"paymentInfo":{"paymentId":"', payment_id, '","amount":', toString(amount), '.0,"currency":"', currency, '",',
            '"country":"', country, '","customerId":null,"udfs":null,"paymentType":"ORDER_PAYMENT","metadata":"{\\\\"segment\\\\":\\\\"', segment, '\\\\"}",',
            '"internalMetadata":null,"isEmi":null,"emiBank":null,"emiTenure":null,"paymentMethodType":"CREDIT","paymentMethod":"CARD",',
            '"paymentSource":null,"authType":null,"cardIssuerBankName":null,"cardIsin":"', bin, '","cardType":"CREDIT","cardSwitchProvider":null,',
            '"cardProgram":null,"cardIssuerCountry":"', country, '","channel":null},"merchantId":"${MERCHANT_ID}",',
            '"eligibleGatewayList":["stripe:mca_stripe","adyen:mca_adyen","checkout:mca_checkout","worldpay:mca_worldpay"],',
            '"rankingAlgorithm":"SR_BASED_ROUTING","eliminationEnabled":null,"enableMultiObjective":null}},',
            '"response":{"static_routing":{"payment_id":"', payment_id, '","status":"success","output":{"type":"priority","connectors":[]},',
            '"evaluated_output":[{"gateway_name":"', gateway, '","gateway_id":"mca_', gateway, '"}]},',
            '"dynamic_routing":{"status":"success","decision":{"decided_gateway":"', gateway, ':mca_', gateway, '",',
            '"gateway_priority_map":{"stripe:mca_stripe":', toString(score1), ',"adyen:mca_adyen":', toString(score2), ',"checkout:mca_checkout":0.91,"worldpay:mca_worldpay":0.88},',
            '"routing_approach":"SR_SELECTION_V3_ROUTING","reset_approach":"NO_RESET","routing_dimension":"ORDER_PAYMENT, CREDIT, CARD",',
            '"routing_dimension_level":"PM_LEVEL","is_rust_based_decider":true,"latency":4,"debug":"', filler, '"}}},',
            '"selection_reason":{"static_status":"success","dynamic_status":"success"}}'),
        kind = 1 AND endpoint = 'gateway', concat(
            '{"request":{"paymentInfo":{"paymentId":"', payment_id, '","amount":', toString(amount), '.0,"currency":"', currency, '",',
            '"country":"', country, '","customerId":null,"udfs":null,"paymentType":"ORDER_PAYMENT","metadata":"{\\\\"segment\\\\":\\\\"', segment, '\\\\"}",',
            '"internalMetadata":null,"isEmi":false,"emiBank":null,"emiTenure":null,"paymentMethodType":"CREDIT","paymentMethod":"CARD",',
            '"paymentSource":null,"authType":null,"cardIssuerBankName":null,"cardIsin":"', bin, '","cardType":"CREDIT","cardSwitchProvider":null,',
            '"cardProgram":null,"cardIssuerCountry":"', country, '","channel":null},"merchantId":"${MERCHANT_ID}",',
            '"eligibleGatewayList":["stripe","adyen","checkout","worldpay"],"rankingAlgorithm":"SR_BASED_ROUTING","eliminationEnabled":true,"enableMultiObjective":null},',
            '"response":{"decided_gateway":"', gateway, '","fallback_gateways":["stripe","adyen"],',
            '"gateway_priority_map":{"stripe":', toString(score1), ',"adyen":', toString(score2), ',"checkout":0.91,"worldpay":0.88},',
            '"routing_approach":"SR_SELECTION_V3_ROUTING","reset_approach":"NO_RESET","routing_dimension":"ORDER_PAYMENT, CREDIT, CARD",',
            '"routing_dimension_level":"PM_LEVEL","is_scheduled_outage":false,"is_rust_based_decider":true,"latency":6,"debug":"',
            substring(filler, 1, 500), '"}}'),
        kind = 1, concat(
            '{"request":{"payment_id":"', payment_id, '","created_by":"${MERCHANT_ID}","fallback_output":null,',
            '"parameters":{"card_bin":{"type":"str_value","value":"', bin, '"},"billing_country":{"type":"enum_variant","value":"', country_name, '"},',
            '"amount":{"type":"number","value":', toString(amount), '},"segment":{"type":"metadata_variant","value":{"key":"segment","value":"', segment, '"}}}},',
            '"response":{"payment_id":"', payment_id, '","status":"success","output":{"type":"priority","connectors":[{"gateway_name":"', gateway,
            '","gateway_id":"mca_', gateway, '"}]},"evaluated_output":[],"eligible_connectors":[]},"rule_name":"Card Rule","preview_kind":"routing_evaluate"}'),
        kind = 3, concat(
            '{"request":{"merchant_id":"${MERCHANT_ID}","gateway":"', gateway, '","payment_id":"', payment_id, '","status":"CHARGED",',
            '"gateway_reference_id":null,"enforce_dynamic_routing_failure":null,"txn_latency":null,"error_info":null,"is_smart_retry":null},',
            '"response":{"message":"Gateway score updated successfully","merchant_id":"${MERCHANT_ID}","gateway":"', gateway,
            '","payment_id":"', payment_id, '"},"selection_reason":{"transaction_status":"CHARGED","stage":"gateway score updated"}}'),
        concat('{"gateway":"', gateway, '","score":', toString(score1), ',"transaction_count":120}')) AS details,
    toInt64(${start_ms} + number * ${span_ms} / ${payments} + kind * 40) AS created_at_ms
FROM (
    SELECT
        number,
        -- Random like Hyperswitch's ids, so the sort order (merchant, lookup_key, time) does not
        -- follow time.
        concat('pay_', lower(hex(cityHash64(number, '${MERCHANT_ID}')))) AS payment_id,
        multiIf(number % 10 < 4, 'decide', number % 10 < 6, 'hybrid', number % 10 < 9, 'gateway', 'evaluate') AS endpoint,
        -- Skewed: most payments carry a few BINs.
        toString(400000 + toUInt32(floor(pow(cityHash64(number, 'bin') % 1000000 / 1000000, 3) * 200)) * 37) AS bin,
        cityHash64(number, 'country') % 10 + 1 AS country_index,
        countries[country_index] AS country,
        country_names[country_index] AS country_name,
        currencies[country_index] AS currency,
        segments[cityHash64(number, 'segment') % 4 + 1] AS segment,
        gateways[cityHash64(number, 'gateway') % 4 + 1] AS gateway,
        cityHash64(number, 'amount') % 50000 + 100 AS amount,
        round(0.9 + (cityHash64(number, 's1') % 100) / 1000, 3) AS score1,
        round(0.9 + (cityHash64(number, 's2') % 100) / 1000, 3) AS score2,
        -- Incompressible-ish scores and diagnostics, so stored size matches the live ~5 KB.
        arrayStringConcat(arrayMap(j -> hex(cityHash64(number, j)), range(160))) AS filler
    FROM numbers(${payments})
)
ARRAY JOIN if(endpoint = 'evaluate', [0, 1], [0, 1, 2, 3, 4]) AS kind;
SQL

echo "Filling the audit summary tables..."
source_rows="SELECT merchant_id, lookup_key AS effective_lookup_key, summary_kind, created_at_ms, payment_id,
    request_id, status, gateway, event_stage, route, flow_type, error_code
    FROM analytics_domain_events WHERE merchant_id = '${MERCHANT_ID}' AND lookup_key != ''"
states="minState(created_at_ms), maxState(created_at_ms), sumState(toUInt64(1)),
    argMaxState(payment_id, created_at_ms), argMaxState(request_id, created_at_ms),
    argMaxState(merchant_id, created_at_ms), argMaxState(status, created_at_ms),
    argMaxState(gateway, created_at_ms), argMaxState(event_stage, created_at_ms),
    groupUniqArrayState(ifNull(gateway, '')), groupUniqArrayState(ifNull(route, '')),
    groupUniqArrayState(ifNull(status, '')), groupUniqArrayState(flow_type),
    groupUniqArrayState(ifNull(error_code, ''))"
# One day at a time, as the live pipeline writes small batches: the summaries are aggregate
# states, which ClickHouse merges across inserts, and one statement over every payment would need
# memory for all of them at once.
for day in $(seq 0 $((DAYS - 1))); do
  day_rows="${source_rows} AND created_at_ms >= $((start_ms + day * 86400000)) AND created_at_ms < $((start_ms + (day + 1) * 86400000))"
  ch <<SQL
INSERT INTO analytics_payment_audit_summary_buckets
SELECT merchant_id, effective_lookup_key, summary_kind,
    toStartOfFifteenMinutes(fromUnixTimestamp64Milli(created_at_ms)) AS bucket_start, ${states}
FROM (${day_rows}) WHERE summary_kind != ''
GROUP BY merchant_id, effective_lookup_key, summary_kind, bucket_start;

INSERT INTO analytics_payment_audit_lookup_summaries
SELECT merchant_id, effective_lookup_key, summary_kind, ${states}
FROM (${day_rows}) WHERE summary_kind != ''
GROUP BY merchant_id, effective_lookup_key, summary_kind;
SQL
done

# Merge the merchant's partitions fully, so every run reads the same layout: a month of
# long-lived data is merged, and freshly inserted parts (each a slice of time ClickHouse can skip
# whole) would flatter the results.
echo "Merging the seeded partitions..."
for partition in $(ch -q "SELECT DISTINCT partition FROM system.parts WHERE active AND table = 'analytics_domain_events' AND partition IN (SELECT DISTINCT toString(toYYYYMM(created_at)) FROM analytics_domain_events WHERE merchant_id = '${MERCHANT_ID}')"); do
  ch -q "OPTIMIZE TABLE analytics_domain_events PARTITION ${partition} FINAL"
done

ch <<SQL
SELECT 'events', count(), formatReadableSize(sum(length(details))),
    formatReadableSize(sumIf(length(details), flow_type = 'routing_hybrid_decision') / countIf(flow_type = 'routing_hybrid_decision'))
FROM analytics_domain_events WHERE merchant_id = '${MERCHANT_ID}' FORMAT TSV;
SQL
