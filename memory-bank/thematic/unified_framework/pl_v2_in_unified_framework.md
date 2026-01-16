# PL_v2 (Euclid) in the Unified Drools-like Framework

## Current PL_v2 Architecture

The current Euclid Rule Engine (PL_v2) is structured as:

1. **AST Module**: Defines the structure of rules and conditions
2. **Interpreter**: Evaluates rules against transaction context
3. **Output Types**: 
   - Priority (ordered list)
   - VolumeSplit (probabilistic selection)
   - VolumeSplitPriority (probabilistic selection between priority lists)
4. **Rule Evaluation Flow**: 
   - Match conditions against context
   - Select first matching rule
   - Apply rule's output specification

## Transformation into Unified Framework

In the unified Drools-like framework, PL_v2 would be restructured while preserving its functionality:

### 1. Euclid DSL as a Filter Component

The most significant change would be transforming Euclid's rule matching engine into a filter component:

```json
{
  "id": "euclid_rule_filter",
  "type": "filter",
  "implementation": "EuclidRuleFilter",
  "config": {
    "rules": [
      {
        "name": "High Value Transaction Rule",
        "condition": [
          {
            "lhs": "order.amount",
            "comparison": "greater_than",
            "value": {"type": "number", "value": 5000}
          }
        ],
        "tag": "high_value"
      },
      {
        "name": "Premium Card Rule",
        "condition": [
          {
            "lhs": "payment.method.cardBrand",
            "comparison": "equal",
            "value": {"type": "enum_variant", "value": "AMEX"}
          }
        ],
        "tag": "premium_card"
      }
    ]
  }
}
```

This filter would add tags to the working memory based on which rules match, rather than directly determining gateway selection.

### 2. Euclid Output Types as Output Processors

The various output types would become output processors in the unified framework:

```json
{
  "id": "priority_output_processor",
  "type": "output_processor",
  "implementation": "PriorityListProcessor",
  "config": {
    "rules": [
      {
        "tag": "high_value",
        "output": {
          "type": "priority",
          "gateways": ["stripe", "adyen", "razorpay"]
        }
      },
      {
        "tag": "premium_card",
        "output": {
          "type": "priority",
          "gateways": ["amex_gateway", "stripe", "adyen"]
        }
      }
    ],
    "default_output": {
      "type": "priority",
      "gateways": ["razorpay", "cashfree", "payu"]
    }
  }
}
```

```json
{
  "id": "volume_split_processor",
  "type": "output_processor",
  "implementation": "VolumeSplitProcessor",
  "config": {
    "rules": [
      {
        "tag": "high_value",
        "output": {
          "type": "volume_split",
          "splits": [
            {"gateway": "stripe", "weight": 70},
            {"gateway": "adyen", "weight": 30}
          ]
        }
      }
    ],
    "default_output": {
      "type": "volume_split",
      "splits": [
        {"gateway": "razorpay", "weight": 50},
        {"gateway": "payu", "weight": 30},
        {"gateway": "cashfree", "weight": 20}
      ]
    }
  }
}
```

### 3. Euclid as a Routing Algorithm

The complete Euclid rule engine would become a routing algorithm combining the filter and output processor:

```json
{
  "id": "euclid_routing_algorithm",
  "name": "Euclid DSL Routing",
  "description": "Gateway routing using Euclid rule language",
  "filters": ["euclid_rule_filter"],
  "comparators": [],
  "output_processor": {
    "type": "conditional",
    "processors": {
      "priority": "priority_output_processor",
      "volume_split": "volume_split_processor",
      "volume_split_priority": "volume_split_priority_processor"
    }
  }
}
```

## Key Improvements

### 1. Rule Reusability

The Euclid rule conditions and output specifications would be decoupled, allowing conditions to be reused across different output strategies:

```json
{
  "id": "multi_strategy_routing",
  "description": "Uses the same rule conditions for different routing strategies",
  "filters": ["euclid_rule_filter"],
  "strategies": [
    {
      "name": "Daytime Strategy",
      "condition": "context.time_of_day == 'DAYTIME'",
      "output_processor": "priority_output_processor"
    },
    {
      "name": "Nighttime Strategy",
      "condition": "context.time_of_day == 'NIGHTTIME'",
      "output_processor": "volume_split_processor"
    }
  ]
}
```

### 2. Composition with Other Components

Euclid rules could be combined with other filter types:

```json
{
  "id": "hybrid_routing_algorithm",
  "name": "Hybrid Euclid + Success Rate Routing",
  "filters": [
    "euclid_rule_filter",
    "elimination_threshold_filter"
  ],
  "comparators": [
    {
      "id": "success_rate_comparator",
      "condition": "!context.rule_tags.contains('premium_card')"
    }
  ],
  "output_processor": "volume_split_processor"
}
```

### 3. Enhanced Rule Development Experience

1. **Rule Testing**: Each rule condition can be unit tested independently
2. **Rule Analytics**: Track which rules are being activated and their impact
3. **Visual Rule Builder**: Create a UI for building Euclid filters and outputs

## Rule Versioning and Deployment

```json
{
  "id": "euclid_rule_set_v1",
  "version": "1.0.0",
  "active": true,
  "merchant_id": "merchant_123",
  "filters": [
    {
      "id": "euclid_rule_filter",
      "version": "1.0.0",
      "config": {
        "rules": [
          {
            "name": "International Transaction Rule",
            "condition": [
              {
                "lhs": "payment.country",
                "comparison": "not_equal",
                "value": {"type": "string", "value": "IN"}
              }
            ],
            "tag": "international"
          }
        ]
      }
    }
  ],
  "output_processors": [
    {
      "id": "priority_output_processor",
      "version": "1.0.0",
      "config": {
        "rules": [
          {
            "tag": "international",
            "output": {
              "type": "priority",
              "gateways": ["stripe", "adyen", "paypal"]
            }
          }
        ]
      }
    }
  ]
}
```

## Migration Pathway

1. **Create Adapters**: Build adapters that transform existing Euclid rules to the new format
2. **Dual Execution**: Run both systems in parallel to validate results
3. **Gradual Migration**: Move merchants one by one to the new system
4. **Legacy Support**: Maintain backward compatibility for existing rule definitions

## Example: International Transaction Rule

### Current Euclid Implementation

```json
{
  "rules": [
    {
      "name": "International Transaction Rule",
      "routing_type": "priority",
      "output": {
        "Priority": ["stripe", "adyen", "paypal"]
      },
      "statements": [
        {
          "condition": [
            {
              "lhs": "payment.cardIssuerCountry",
              "comparison": "not_equal",
              "value": {"type": "string_value", "value": "IN"},
              "metadata": {}
            }
          ],
          "nested": null
        }
      ]
    }
  ],
  "default_selection": {
    "Priority": ["razorpay", "payu", "cashfree"]
  }
}
```

### New Unified Framework Implementation

```json
{
  "routing_algorithm": {
    "id": "international_routing",
    "filters": [
      {
        "id": "euclid_rule_filter",
        "config": {
          "rules": [
            {
              "name": "International Transaction Rule",
              "condition": [
                {
                  "lhs": "payment.cardIssuerCountry",
                  "comparison": "not_equal",
                  "value": {"type": "string_value", "value": "IN"}
                }
              ],
              "tag": "international"
            }
          ]
        }
      }
    ],
    "output_processor": {
      "id": "priority_output_processor",
      "config": {
        "rules": [
          {
            "tag": "international",
            "output": {
              "type": "priority",
              "gateways": ["stripe", "adyen", "paypal"]
            }
          }
        ],
        "default_output": {
          "type": "priority",
          "gateways": ["razorpay", "payu", "cashfree"]
        }
      }
    }
  }
}
```

## Technical Implementation Considerations

1. **Rule Compilation**: Convert JSON rule definitions to efficient in-memory structures
2. **Working Memory Integration**: Insert Euclid rule results into the shared working memory
3. **Performance Optimization**: Ensure rule evaluation remains sub-millisecond
4. **Rule Management API**: Provide interfaces for CRUD operations on rules

## Benefits for Euclid Integration

1. **Enhanced Flexibility**: Mix and match Euclid rules with other routing components
2. **Better Testability**: Isolate rule condition evaluation from gateway selection logic
3. **Streamlined Configuration**: Unified configuration format across routing types
4. **Progressive Enhancement**: Start with pure Euclid functionality and gradually add features
5. **Data-Driven Improvements**: Collect analytics on rule effectiveness
