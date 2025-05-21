# Success Rate Based Routing in the Unified Drools-like Framework

## Current Success Rate Routing Architecture

The current Success Rate based routing approach in the Decision Engine is structured as:

1. **Gateway Success Metrics Collection**: Recording transaction outcomes for various gateways
2. **Elimination Thresholds**: Removing gateways that fall below specified success rates
3. **Multi-dimensional Analysis**: Analyzing success rates across different dimensions (card type, amount, etc.)
4. **Weighted Gateway Selection**: Ranking gateways based on historical performance

## Transformation into Unified Framework

In the unified Drools-like framework, Success Rate routing would be restructured into composable components:

### 1. Success Rate Filters

Filters that eliminate underperforming gateways based on historical success rates:

```json
{
  "id": "elimination_threshold_filter",
  "type": "filter",
  "implementation": "SuccessRateThresholdFilter",
  "config": {
    "global_threshold": 0.75,
    "gateway_specific_thresholds": {
      "stripe": 0.80,
      "razorpay": 0.70
    },
    "dimensions": ["card_type", "card_brand", "amount_range"],
    "lookback_period_days": 7,
    "minimum_transaction_count": 50
  }
}
```

### 2. Success Rate Comparators

Comparators that rank gateways based on their performance metrics:

```json
{
  "id": "success_rate_comparator",
  "type": "comparator",
  "implementation": "SuccessRateComparator",
  "config": {
    "dimensions": [
      {
        "name": "card_brand",
        "weight": 0.4,
        "min_sample_size": 20
      },
      {
        "name": "card_country",
        "weight": 0.3,
        "min_sample_size": 15
      },
      {
        "name": "amount_range",
        "weight": 0.2,
        "min_sample_size": 10
      },
      {
        "name": "time_of_day",
        "weight": 0.1,
        "min_sample_size": 5
      }
    ],
    "fallback_strategy": "global_success_rate"
  }
}
```

```json
{
  "id": "latency_comparator",
  "type": "comparator",
  "implementation": "LatencyComparator",
  "config": {
    "lookback_period_hours": 24,
    "percentile": 95,
    "min_sample_size": 30
  }
}
```

### 3. Success Rate Routing Algorithm

The complete Success Rate routing algorithm combining filters and comparators:

```json
{
  "id": "success_rate_routing_algorithm",
  "name": "Success Rate Based Routing",
  "description": "Routes transactions based on historical gateway performance",
  "filters": [
    "elimination_threshold_filter",
    "payment_method_compatibility_filter"
  ],
  "comparators": [
    {
      "id": "success_rate_comparator",
      "weight": 0.7
    },
    {
      "id": "latency_comparator",
      "weight": 0.3
    }
  ],
  "output_processor": "ranked_list_processor"
}
```

## Integration with Working Memory

For success rate routing to work efficiently in the Drools system, the working memory would contain:

### 1. Transaction Context Facts

```java
// Example facts for working memory
fact TransactionRequest {
    String transactionId;
    double amount;
    String currency;
    String cardBrand;
    String cardType;
    String cardIssuerCountry;
}

fact GatewaySuccessMetrics {
    String gatewayId;
    Map<String, Double> dimensionSuccessRates;
    double overallSuccessRate;
    timestamp lastUpdated;
    int transactionCount;
}

fact GatewayLatencyMetrics {
    String gatewayId;
    double p95LatencyMs;
    double p99LatencyMs;
    double averageLatencyMs;
    timestamp lastUpdated;
}
```

### 2. Rule Patterns

```java
// Example Drools rule for success rate elimination
rule "Eliminate Underperforming Gateways"
when
    $tx: TransactionRequest()
    $gateway: Gateway()
    $metrics: GatewaySuccessMetrics(
        gatewayId == $gateway.id,
        transactionCount >= 50,
        overallSuccessRate < 0.75
    )
then
    insertLogical(new GatewayEliminationResult(
        $gateway.id,
        "below_threshold",
        $metrics.overallSuccessRate,
        0.75
    ));
end

// Example Drools rule for gateway ranking
rule "Rank Gateways By Success Rate"
salience 100
when
    $tx: TransactionRequest()
    $g1: Gateway()
    $g2: Gateway(id != $g1.id)
    $m1: GatewaySuccessMetrics(gatewayId == $g1.id)
    $m2: GatewaySuccessMetrics(gatewayId == $g2.id)
    eval($m1.overallSuccessRate > $m2.overallSuccessRate)
then
    insertLogical(new GatewayPreference($g1, $g2, "success_rate"));
end
```

## Feedback Mechanism

The feedback loop would be integrated into the working memory system:

```json
{
  "id": "transaction_feedback_processor",
  "type": "feedback_processor",
  "implementation": "SuccessRateFeedbackProcessor",
  "config": {
    "dimensions": ["card_brand", "card_type", "amount_range", "time_of_day"],
    "decay_factor": 0.95,
    "aggregation_intervals": ["1h", "24h", "7d", "30d"],
    "cache_ttl_seconds": 3600
  }
}
```

## Example: Card Brand Based Success Rate Routing

### Current Implementation

```
// Pseudocode representation of current approach
function decideGateway(transaction) {
  eligibleGateways = getAllEligibleGateways(transaction)
  
  // Remove gateways below threshold
  filteredGateways = eliminateLowPerformingGateways(eligibleGateways, 0.75)
  
  // Get success rates for relevant dimension
  cardBrand = transaction.cardBrand
  successRates = getGatewaySuccessRatesByDimension(filteredGateways, "card_brand", cardBrand)
  
  // Rank gateways by success rate
  rankedGateways = sortBySuccessRate(successRates)
  
  return rankedGateways[0]
}
```

### New Unified Framework Implementation

```json
{
  "routing_algorithm": {
    "id": "card_brand_success_rate_routing",
    "filters": [
      {
        "id": "elimination_threshold_filter",
        "config": {
          "global_threshold": 0.75,
          "lookback_period_days": 7
        }
      }
    ],
    "comparators": [
      {
        "id": "card_brand_success_rate_comparator",
        "config": {
          "dimensions": [
            {
              "name": "card_brand",
              "weight": 1.0,
              "min_sample_size": 10
            }
          ],
          "fallback_strategy": "global_success_rate"
        },
        "weight": 1.0
      }
    ],
    "output_processor": "ranked_list_processor"
  }
}
```

## Key Improvements

### 1. Dimension Weighting Flexibility

The new approach allows for more flexible dimension weighting:

```json
{
  "id": "multi_dimension_success_rate_comparator",
  "type": "comparator",
  "config": {
    "dimensions": [
      {"name": "card_brand", "weight": 0.4},
      {"name": "card_issuer_country", "weight": 0.3},
      {"name": "transaction_hour", "weight": 0.2},
      {"name": "amount_range", "weight": 0.1}
    ],
    "dimension_combination_strategy": "weighted_average"
  }
}
```

### 2. Dynamic Dimension Selection

The system could dynamically select which dimensions to use based on available data:

```json
{
  "id": "adaptive_dimension_selector",
  "type": "dimension_selector",
  "implementation": "AdaptiveDimensionSelector",
  "config": {
    "available_dimensions": [
      {"name": "card_brand", "min_sample_size": 20},
      {"name": "card_type", "min_sample_size": 15},
      {"name": "amount_range", "min_sample_size": 30},
      {"name": "time_of_day", "min_sample_size": 50},
      {"name": "card_issuer_country", "min_sample_size": 25}
    ],
    "fallback_order": ["card_brand", "card_type", "global"],
    "sample_size_buffer": 1.2
  }
}
```

### 3. Gateway Group Normalization

Success rates could be normalized across gateway groups:

```json
{
  "id": "gateway_group_normalizer",
  "type": "preprocessor",
  "implementation": "GatewayGroupNormalizer",
  "config": {
    "gateway_groups": [
      {
        "name": "stripe_group",
        "gateways": ["stripe", "stripe_direct", "stripe_india"]
      },
      {
        "name": "razorpay_group",
        "gateways": ["razorpay", "razorpay_direct"]
      }
    ],
    "normalization_strategy": "max_success_rate"
  }
}
```

## Transition Strategy

To migrate from the current success rate implementation to the unified framework:

1. **Component Mapping**: Map current success rate logic to filter and comparator components
2. **Data Migration**: Ensure historical success rate data is accessible in the unified system
3. **Parallel Validation**: Run both implementations side by side to validate results
4. **Component Testing**: Verify each filter and comparator independently
5. **Gradual Rollout**: Deploy to production in phases, monitoring performance

## Benefits for Success Rate Routing

1. **Improved Adaptability**: Easily combine with other routing criteria
2. **Better Explainability**: Clear separation of filtering and ranking logic
3. **Enhanced Testability**: Test each component in isolation
4. **Configuration Flexibility**: Adjust dimension weights without code changes
5. **Evolutionary Approach**: Start with simple implementation and gradually add sophistication

## Technical Implementation Notes

1. **Caching Strategy**: Cache success rate metrics for fast access
2. **Precomputation**: Calculate common dimension values during off-peak hours
3. **Fast Updates**: Update working memory efficiently when new transaction outcomes arrive
4. **Fallback Mechanisms**: Define clear fallback paths when data is insufficient

## Example Use Case: International E-commerce

For a merchant handling international transactions with various card brands:

```json
{
  "decider": {
    "id": "international_ecommerce_decider",
    "routing_algorithms": [
      {
        "id": "premium_card_routing",
        "filters": [
          {
            "id": "premium_card_filter",
            "config": {
              "premium_cards": ["AMEX", "VISA_INFINITE", "MASTERCARD_WORLD"]
            }
          }
        ],
        "comparators": [],
        "output_processor": "priority_output_processor",
        "conditions": "transaction.card_brand in ['AMEX', 'VISA_INFINITE', 'MASTERCARD_WORLD']"
      },
      {
        "id": "success_rate_routing",
        "filters": [
          "elimination_threshold_filter"
        ],
        "comparators": [
          {
            "id": "success_rate_comparator",
            "weight": 0.7
          },
          {
            "id": "latency_comparator",
            "weight": 0.3
          }
        ],
        "output_processor": "ranked_list_processor",
        "conditions": "true" // Default case
      }
    ],
    "selection_strategy": "first_matching_condition"
  }
}
