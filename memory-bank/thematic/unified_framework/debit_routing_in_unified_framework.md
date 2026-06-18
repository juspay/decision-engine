# Debit Routing in the Unified Drools-like Framework

## Current Debit Routing Architecture

The current Debit Network Routing in the Decision Engine is designed for optimizing debit card transactions across different card networks:

1. **Network-Based Selection**: Routing based on debit card networks (RuPay, Visa, Mastercard, etc.)
2. **Bank-Specific Preferences**: Customized routing for different issuing banks
3. **Co-branded Card Handling**: Special logic for cards that support multiple networks
4. **Fallback Mechanisms**: Alternative paths when preferred networks are unavailable

## Transformation into Unified Framework

In the unified Drools-like framework, Debit Routing would be restructured while preserving its specialized functionality:

### 1. Network Detection as Filter Component

```json
{
  "id": "debit_network_filter",
  "type": "filter",
  "implementation": "DebitNetworkFilter",
  "config": {
    "network_detection": {
      "bin_ranges": {
        "rupay": ["508500-508999", "607000-607199", "608000-608199"],
        "visa": ["400000-499999"],
        "mastercard": ["510000-559999"]
      },
      "co_branded_detection": true
    }
  }
}
```

### 2. Bank-specific Preferences as Filter Component

```json
{
  "id": "bank_preference_filter",
  "type": "filter",
  "implementation": "BankPreferenceFilter",
  "config": {
    "bank_preferences": {
      "HDFC": {
        "preferred_networks": ["rupay", "visa", "mastercard"]
      },
      "SBI": {
        "preferred_networks": ["rupay", "mastercard", "visa"]
      },
      "ICICI": {
        "preferred_networks": ["visa", "rupay", "mastercard"]
      }
    },
    "default_preference": ["rupay", "visa", "mastercard"]
  }
}
```

### 3. Network to Gateway Mapping Component

```json
{
  "id": "network_gateway_mapper",
  "type": "mapper",
  "implementation": "NetworkGatewayMapper",
  "config": {
    "network_gateway_map": {
      "rupay": ["razorpay", "cashfree", "paytm"],
      "visa": ["stripe", "razorpay", "adyen"],
      "mastercard": ["stripe", "adyen", "razorpay"]
    },
    "gateway_capabilities": {
      "razorpay": ["rupay", "visa", "mastercard"],
      "stripe": ["visa", "mastercard"],
      "adyen": ["visa", "mastercard"],
      "cashfree": ["rupay", "visa", "mastercard"],
      "paytm": ["rupay"]
    }
  }
}
```

### 4. Debit Routing as a Complete Algorithm

```json
{
  "id": "debit_network_routing",
  "name": "Debit Card Network Routing",
  "description": "Optimized routing for debit card transactions based on network preferences",
  "filters": [
    "debit_network_filter", 
    "bank_preference_filter"
  ],
  "processors": [
    "network_gateway_mapper"
  ],
  "output_processor": {
    "id": "prioritized_network_processor",
    "implementation": "PrioritizedNetworkProcessor",
    "config": {
      "fallback_strategy": "next_preferred_network",
      "international_fallback": ["visa", "mastercard"]
    }
  }
}
```

## Working Memory Integration

For debit routing to integrate properly with the Drools-like working memory:

```java
// Facts for Debit Routing
fact DebitCardInfo {
    String cardBin;
    List<String> supportedNetworks;
    boolean isCobranded;
    String issuingBank;
    String preferredNetwork;
}

fact NetworkMetrics {
    String networkName;
    double successRate;
    double averageLatencyMs;
    timestamp lastUpdated;
}

fact NetworkPreference {
    String bank;
    List<String> preferredNetworkOrder;
}

fact DebitRoutingResult {
    String selectedNetwork;
    List<String> eligibleGateways;
    String selectedGateway;
    String routingReason;
}
```

## Rule Patterns

```java
// Example Drools rule for network selection
rule "Select Preferred Network for Bank"
when
    $tx: TransactionRequest(paymentMethod == "DEBIT_CARD")
    $cardInfo: DebitCardInfo()
    $bankPref: NetworkPreference(bank == $cardInfo.issuingBank)
then
    List<String> availableNetworks = new ArrayList<>(
        $cardInfo.supportedNetworks
    );
    String selectedNetwork = null;
    
    // Try each preferred network in order
    for (String network : $bankPref.preferredNetworkOrder) {
        if (availableNetworks.contains(network)) {
            selectedNetwork = network;
            break;
        }
    }
    
    if (selectedNetwork == null && !availableNetworks.isEmpty()) {
        selectedNetwork = availableNetworks.get(0);
    }
    
    insertLogical(new DebitRoutingResult(
        selectedNetwork, 
        Collections.emptyList(), 
        null,
        "bank_preference"
    ));
end
```

## Key Improvements

### 1. Dynamic Network Success Rates

The unified framework would allow for incorporating success rate data into network selection:

```json
{
  "id": "network_success_rate_comparator",
  "type": "comparator",
  "implementation": "NetworkSuccessRateComparator",
  "config": {
    "lookback_period_days": 7,
    "minimum_transaction_count": 30,
    "success_rate_threshold": 0.85,
    "dimensions": [
      {
        "name": "bank",
        "weight": 0.7
      },
      {
        "name": "card_bin_range",
        "weight": 0.3
      }
    ]
  }
}
```

### 2. Co-branded Card Intelligence

Enhanced logic for co-branded card routing:

```json
{
  "id": "cobranded_card_optimizer",
  "type": "processor",
  "implementation": "CobrandedCardOptimizer",
  "config": {
    "default_network_priority": {
      "rupay_visa": "rupay",
      "rupay_mastercard": "rupay",
      "visa_mastercard": "visa"
    },
    "bank_specific_overrides": {
      "HDFC": {
        "rupay_visa": "visa"
      }
    },
    "success_rate_differential_threshold": 0.05,
    "use_success_rate_when_available": true
  }
}
```

### 3. Gateway Performance Integration

```json
{
  "id": "debit_network_with_gateway_performance",
  "name": "Network-Gateway Combined Optimization",
  "filters": [
    "debit_network_filter",
    "bank_preference_filter",
    "elimination_threshold_filter"
  ],
  "processors": [
    "network_gateway_mapper"
  ],
  "comparators": [
    {
      "id": "gateway_success_rate_comparator",
      "weight": 0.7
    },
    {
      "id": "gateway_latency_comparator",
      "weight": 0.3
    }
  ],
  "output_processor": "ranked_networks_processor"
}
```

## Example: Domestic Debit Card Transaction

### Current Implementation (Pseudocode)

```
function decideNetwork(transaction) {
  // Detect card networks
  networks = detectNetworks(transaction.cardBin)
  
  // Check if co-branded
  isCobranded = (networks.size > 1)
  
  // Get bank preferences
  bankPreferences = getBankPreferences(transaction.issuingBank)
  
  if (isCobranded) {
    // Handle co-branded card
    preferredNetwork = getPreferredNetworkForCobrandedCard(networks)
    if (preferredNetwork != null) {
      return getGatewaysForNetwork(preferredNetwork)
    }
  }
  
  // Try bank preferences in order
  for (network in bankPreferences) {
    if (networks.contains(network)) {
      return getGatewaysForNetwork(network)
    }
  }
  
  // Fallback to default
  return getGatewaysForNetwork(networks[0])
}
```

### New Unified Framework Implementation

```json
{
  "routing_algorithm": {
    "id": "domestic_debit_routing",
    "filters": [
      {
        "id": "debit_network_filter",
        "config": {
          "network_detection": {
            "bin_ranges": {
              "rupay": ["508500-508999", "607000-607199", "608000-608199"],
              "visa": ["400000-499999"],
              "mastercard": ["510000-559999"]
            },
            "co_branded_detection": true
          }
        }
      },
      {
        "id": "bank_preference_filter",
        "config": {
          "bank_preferences": {
            "HDFC": {
              "preferred_networks": ["rupay", "visa", "mastercard"]
            },
            "SBI": {
              "preferred_networks": ["rupay", "mastercard", "visa"]
            },
            "ICICI": {
              "preferred_networks": ["visa", "rupay", "mastercard"]
            }
          },
          "default_preference": ["rupay", "visa", "mastercard"]
        }
      }
    ],
    "processors": [
      {
        "id": "cobranded_card_optimizer",
        "config": {
          "default_network_priority": {
            "rupay_visa": "rupay",
            "rupay_mastercard": "rupay",
            "visa_mastercard": "visa"
          }
        }
      },
      {
        "id": "network_gateway_mapper",
        "config": {
          "network_gateway_map": {
            "rupay": ["razorpay", "cashfree", "paytm"],
            "visa": ["stripe", "razorpay", "adyen"],
            "mastercard": ["stripe", "adyen", "razorpay"]
          }
        }
      }
    ],
    "output_processor": {
      "id": "prioritized_network_processor",
      "config": {
        "fallback_strategy": "next_preferred_network"
      }
    }
  }
}
```

## Integration with Other Routing Algorithms

The unified framework allows debit routing to be integrated with other routing strategies:

### 1. Debit Network + Success Rate Hybrid

```json
{
  "id": "debit_network_success_rate_hybrid",
  "description": "Select network by preference, then optimize gateway by success rate",
  "filters": [
    "debit_network_filter",
    "bank_preference_filter"
  ],
  "processors": [
    "network_gateway_mapper"
  ],
  "comparators": [
    {
      "id": "success_rate_comparator",
      "config": {
        "dimensions": [
          {"name": "card_network", "weight": 0.5},
          {"name": "bank", "weight": 0.3},
          {"name": "amount_range", "weight": 0.2}
        ]
      },
      "weight": 1.0
    }
  ],
  "output_processor": "ranked_list_processor"
}
```

### 2. Comprehensive Decision Tree

```json
{
  "id": "comprehensive_routing",
  "description": "Multi-stage decision process for all payment methods",
  "decider": {
    "type": "decision_tree",
    "nodes": [
      {
        "condition": "transaction.paymentMethod == 'DEBIT_CARD'",
        "routing_algorithm": "debit_network_routing"
      },
      {
        "condition": "transaction.paymentMethod == 'CREDIT_CARD'",
        "routing_algorithm": "credit_card_success_rate_routing"
      },
      {
        "condition": "transaction.paymentMethod == 'UPI'",
        "routing_algorithm": "upi_routing"
      },
      {
        "default": true,
        "routing_algorithm": "general_success_rate_routing"
      }
    ]
  }
}
```

## Network-Aware Success Rate Analysis

The unified framework would enable more sophisticated analysis that accounts for networks:

```json
{
  "id": "network_aware_success_analysis",
  "type": "analyzer",
  "implementation": "NetworkSuccessAnalyzer",
  "config": {
    "dimensions": [
      {
        "name": "card_network",
        "granularity": ["rupay", "visa", "mastercard"]
      },
      {
        "name": "bank_network_pair",
        "granularity": ["HDFC_rupay", "HDFC_visa", "SBI_rupay", "SBI_visa"]
      },
      {
        "name": "gateway_network_pair",
        "granularity": ["razorpay_rupay", "razorpay_visa", "stripe_visa"]
      }
    ],
    "metrics": [
      "success_rate",
      "average_response_time",
      "decline_rate_by_reason"
    ],
    "time_aggregations": ["1d", "7d", "30d"]
  }
}
```

## Migration Strategy

### 1. Component Mapping

Map existing debit routing logic to the new framework:

```
Debit Routing Components:
- Network detection → debit_network_filter
- Bank preferences → bank_preference_filter
- Co-branded card logic → cobranded_card_optimizer
- Network-to-gateway mapping → network_gateway_mapper
- Default ordering → prioritized_network_processor
```

### 2. Phased Rollout

```json
{
  "rollout_plan": {
    "phases": [
      {
        "name": "Initial Component Development",
        "components": [
          "debit_network_filter",
          "bank_preference_filter",
          "network_gateway_mapper",
          "prioritized_network_processor"
        ],
        "completion_criteria": "All components unit tested and functionally verified"
      },
      {
        "name": "Integration Testing",
        "approach": "Shadow Mode",
        "traffic_percentage": 0,
        "success_criteria": "100% matching with current implementation"
      },
      {
        "name": "Limited Production",
        "traffic_percentage": 5,
        "success_criteria": "Equal or better success rates compared to current implementation"
      },
      {
        "name": "Enhanced Features Rollout",
        "components": [
          "cobranded_card_optimizer",
          "network_success_rate_comparator"
        ],
        "traffic_percentage": 50,
        "success_criteria": "2% improvement in success rate for co-branded cards"
      },
      {
        "name": "Full Migration",
        "traffic_percentage": 100,
        "success_criteria": "All debit transactions using new framework"
      }
    ]
  }
}
```

### 3. Configuration Migration Tool

```json
{
  "migration_tool": {
    "input": {
      "network_preferences": "src/network_decider/debit_routing/network_preferences.json",
      "bank_preferences": "src/network_decider/debit_routing/bank_preferences.json",
      "gateway_mappings": "src/network_decider/debit_routing/gateway_mappings.json"
    },
    "output": {
      "config_type": "unified_framework",
      "output_path": "config/unified/debit_routing_config.json"
    },
    "validation": {
      "run_equivalence_test": true,
      "test_scenarios": 1000,
      "max_acceptable_divergence": 0
    }
  }
}
```

## Benefits for Debit Routing Integration

1. **Enhanced Visibility**: Clear, structured configuration for debit routing logic
2. **Dynamic Optimization**: Incorporate success rates while preserving network preferences
3. **Improved Co-branded Handling**: More sophisticated decision making for multi-network cards
4. **Better Analytics**: Network-specific performance metrics and insights
5. **Seamless Fallbacks**: Structured fallback paths when preferred networks or gateways are unavailable

## Technical Implementation Considerations

1. **BIN Detection Performance**: Fast card network detection with extensive BIN range support
2. **Configuration Complexity Management**: Structured approach to handle complex bank preferences
3. **Co-branded Card Detection Accuracy**: Reliable identification of cards supporting multiple networks
4. **Network-Gateway Relationship Modeling**: Flexible mapping of networks to compatible gateways
5. **International Routing**: Special handling for international debit transactions
