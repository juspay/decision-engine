# PL_v1 (Priority Logic) in the Unified Drools-like Framework

## Current PL_v1 Architecture

The current Priority Logic (PL_v1) in the Decision Engine is structured as:

1. **Groovy Script Execution**: Using the groovy-runner service to execute merchant-specific scripts
2. **Context Mapping**: Transforming transaction details into a format consumable by the scripts
3. **Dynamic Gateway Prioritization**: Scripts producing an ordered list of gateways
4. **Fallback Mechanisms**: Support for fallback logic in case of script execution failures
5. **Caching and Performance Optimizations**: Redis-based caching for commonly used results

## Transformation into Unified Framework

In the unified Drools-like framework, PL_v1 would be restructured while preserving its functionality:

### 1. Groovy Script as a Filter Component

Priority Logic scripts would be transformed into filter components:

```json
{
  "id": "groovy_script_filter",
  "type": "filter",
  "implementation": "GroovyScriptFilter",
  "config": {
    "script_source": "inline",
    "script": "def gatewayPriority = []\n\nif (paymentInfo.cardBrand == 'AMEX') {\n  gatewayPriority = ['stripe', 'adyen']\n  isEnforcement = true\n  return\n}\n\nif (orderInfo.amount > 1000) {\n  gatewayPriority = ['adyen', 'stripe', 'razorpay']\n  return\n}\n\ngatewayPriority = ['razorpay', 'payu', 'cashfree']",
    "error_handling": {
      "retry_count": 1,
      "timeout_ms": 200,
      "circuit_breaker_threshold": 5
    }
  }
}
```

Alternatively, scripts could be stored and referenced by ID:

```json
{
  "id": "groovy_script_filter",
  "type": "filter",
  "implementation": "GroovyScriptFilter",
  "config": {
    "script_source": "merchant_config",
    "script_id": "merchant_123_priority_logic",
    "version": "1.2.0",
    "error_handling": {
      "retry_count": 1,
      "timeout_ms": 200,
      "circuit_breaker_threshold": 5
    }
  }
}
```

### 2. Script Output as Gateway List

The script output would be transformed into a fact in working memory:

```java
fact GroovyScriptResult {
    List<String> gatewayPriority;
    boolean isEnforcement;
    String scriptId;
    String executionStatus;
    long executionTimeMs;
}
```

### 3. PL_v1 as a Routing Algorithm

The complete Priority Logic would become a routing algorithm:

```json
{
  "id": "priority_logic_routing",
  "name": "Merchant Priority Logic Script",
  "description": "Gateway routing using merchant-defined Groovy scripts",
  "filters": ["groovy_script_filter"],
  "comparators": [],
  "output_processor": {
    "id": "script_result_processor",
    "implementation": "ScriptResultProcessor",
    "config": {
      "handle_enforcement": true,
      "fallback_strategy": "default_gateway_list",
      "default_gateway_list": ["razorpay", "payu", "cashfree"]
    }
  }
}
```

## Key Improvements

### 1. Better Error Handling

The unified framework would provide more robust error handling:

```json
{
  "id": "priority_logic_with_fallback",
  "filters": [
    {
      "id": "groovy_script_filter",
      "config": {
        "script_id": "merchant_123_priority_logic",
        "fallback": {
          "on_error": "fallback_script",
          "fallback_script_id": "merchant_123_fallback_logic"
        }
      }
    }
  ],
  "output_processor": "script_result_processor"
}
```

### 2. Script Testing Capabilities

The framework would support script testing and simulation:

```json
{
  "id": "script_test_harness",
  "type": "test_tool",
  "implementation": "ScriptTestHarness",
  "config": {
    "script_id": "merchant_123_priority_logic",
    "test_scenarios": [
      {
        "name": "AMEX Card Test",
        "input": {
          "paymentInfo": {
            "cardBrand": "AMEX",
            "cardType": "CREDIT"
          },
          "orderInfo": {
            "amount": 500.00,
            "currency": "USD"
          }
        },
        "expected_output": {
          "gatewayPriority": ["stripe", "adyen"],
          "isEnforcement": true
        }
      }
    ]
  }
}
```

### 3. Script Versioning and Governance

The framework would provide versioning and approval workflows:

```json
{
  "script_management": {
    "id": "merchant_123_priority_logic",
    "versions": [
      {
        "version": "1.0.0",
        "status": "deprecated",
        "created_by": "john.doe",
        "created_at": "2025-01-15T12:30:00Z",
        "approved_by": "jane.smith",
        "approved_at": "2025-01-16T09:15:00Z"
      },
      {
        "version": "1.1.0",
        "status": "active",
        "created_by": "john.doe",
        "created_at": "2025-03-20T14:45:00Z",
        "approved_by": "jane.smith",
        "approved_at": "2025-03-21T10:30:00Z"
      },
      {
        "version": "1.2.0",
        "status": "pending_approval",
        "created_by": "john.doe",
        "created_at": "2025-05-10T11:00:00Z",
        "changes": "Added support for JCB cards"
      }
    ]
  }
}
```

## Integration with Other Components

The unified framework would allow PL_v1 scripts to be combined with other routing components:

### 1. Hybrid PL_v1 and Success Rate Routing

```json
{
  "id": "hybrid_script_success_rate",
  "name": "Script-first with Success Rate Fallback",
  "filters": [
    "groovy_script_filter",
    "elimination_threshold_filter"
  ],
  "comparators": [
    {
      "id": "success_rate_comparator",
      "condition": "context.script_result == null || context.script_result.gatewayPriority.isEmpty()"
    }
  ],
  "output_processor": {
    "id": "conditional_processor",
    "config": {
      "condition_field": "script_result",
      "condition_test": "!= null && !gatewayPriority.isEmpty()",
      "true_processor": "script_result_processor",
      "false_processor": "ranked_list_processor"
    }
  }
}
```

### 2. Progressive Enhancement

```json
{
  "id": "progressive_enhancement",
  "name": "Progressive Script Enhancement",
  "filters": [
    {
      "id": "groovy_script_filter",
      "config": {
        "script_id": "merchant_123_priority_logic"
      }
    }
  ],
  "comparators": [
    {
      "id": "success_rate_comparator",
      "condition": "true",
      "config": {
        "input_gateway_list": "context.script_result.gatewayPriority",
        "preserve_order": true,
        "reorder_threshold": 0.05
      }
    }
  ],
  "output_processor": "ranked_list_processor"
}
```

## Scripting Environment Enhancements

### 1. Expanded Context Data

The framework would provide richer context to scripts:

```groovy
// Enhanced Groovy script with expanded context
def gatewayPriority = []

// Access to success rate data
def stripeSuccessRate = metrics.getSuccessRate("stripe", "card_brand", paymentInfo.cardBrand)
def adyenSuccessRate = metrics.getSuccessRate("adyen", "card_brand", paymentInfo.cardBrand)

// Access to additional context
def timeOfDay = context.timeOfDay
def customerLTV = context.customerLifetimeValue
def isRecurringCustomer = context.customerTransactionCount > 1

// Make decisions based on richer data
if (customerLTV > 1000 || isRecurringCustomer) {
  // Prioritize reliability for valuable customers
  if (stripeSuccessRate > 0.95) {
    gatewayPriority = ["stripe", "adyen", "razorpay"]
  } else {
    gatewayPriority = ["adyen", "stripe", "razorpay"]
  }
} else {
  // Standard routing
  gatewayPriority = ["razorpay", "cashfree", "payu"]
}

// Return result
return gatewayPriority
```

### 2. Script Libraries and Reusable Functions

The framework would support common function libraries:

```groovy
// Access to shared library functions
import org.juspay.routing.CardUtils
import org.juspay.routing.GatewaySelector

// Use utility functions
def isBinInRange = CardUtils.isCardBinInRange(paymentInfo.cardBin, "400000", "499999")
def isPremiumCard = CardUtils.isPremiumCard(paymentInfo.cardBrand, paymentInfo.cardType)

// Use pre-built selectors
def domesticPreferred = GatewaySelector.buildDomesticPreferredList(availableGateways)
def internationalPreferred = GatewaySelector.buildInternationalPreferredList(availableGateways)

// Decide based on transaction context
if (isPremiumCard) {
  gatewayPriority = internationalPreferred
} else if (paymentInfo.cardIssuerCountry == "IN") {
  gatewayPriority = domesticPreferred
} else {
  gatewayPriority = internationalPreferred
}
```

## Example: International E-commerce Merchant

### Current Implementation

```groovy
// Current PL_v1 script
def gatewayPriority = []

// Card brand specific routing
if (paymentInfo.cardBrand == "AMEX") {
  gatewayPriority = ["stripe", "adyen"]
  isEnforcement = true
  return
}

// Amount based routing
if (orderInfo.amount > 1000) {
  gatewayPriority = ["adyen", "stripe", "razorpay"]
  return
}

// Country based routing
if (paymentInfo.cardIssuerCountry == "US") {
  gatewayPriority = ["stripe", "braintree", "adyen"]
  return
}

if (paymentInfo.cardIssuerCountry == "GB") {
  gatewayPriority = ["adyen", "stripe", "braintree"]
  return
}

// Default routing
gatewayPriority = ["razorpay", "payu", "cashfree"]
```

### New Unified Framework Implementation

```json
{
  "routing_algorithm": {
    "id": "international_ecommerce_routing",
    "filters": [
      {
        "id": "script_condition_filter",
        "implementation": "GroovyConditionFilter",
        "config": {
          "conditions": [
            {
              "name": "AMEX Card",
              "script": "paymentInfo.cardBrand == 'AMEX'",
              "tag": "amex_card"
            },
            {
              "name": "High Value Transaction",
              "script": "orderInfo.amount > 1000",
              "tag": "high_value"
            },
            {
              "name": "US Customer",
              "script": "paymentInfo.cardIssuerCountry == 'US'",
              "tag": "us_customer"
            },
            {
              "name": "UK Customer",
              "script": "paymentInfo.cardIssuerCountry == 'GB'",
              "tag": "uk_customer"
            }
          ]
        }
      }
    ],
    "output_processor": {
      "id": "tag_based_processor",
      "config": {
        "tag_priority": ["amex_card", "high_value", "us_customer", "uk_customer"],
        "tag_outputs": {
          "amex_card": {
            "type": "priority",
            "gateways": ["stripe", "adyen"],
            "enforcement": true
          },
          "high_value": {
            "type": "priority",
            "gateways": ["adyen", "stripe", "razorpay"]
          },
          "us_customer": {
            "type": "priority",
            "gateways": ["stripe", "braintree", "adyen"]
          },
          "uk_customer": {
            "type": "priority",
            "gateways": ["adyen", "stripe", "braintree"]
          }
        },
        "default_output": {
          "type": "priority",
          "gateways": ["razorpay", "payu", "cashfree"]
        }
      }
    }
  }
}
```

## Migration Strategy

### 1. Script Analysis and Pattern Extraction

The first step would be to analyze existing scripts to identify common patterns:

```
Step 1: Identify common decision factors
- Card brand based decisions (AMEX, VISA, etc.)
- Amount-based thresholds
- Country-based routing
- Default fallback options

Step 2: Map to unified framework components
- Decision factors → Filter components
- Gateway priority lists → Output configurations
- Enforcement logic → Output processor settings
```

### 2. Script-to-Configuration Converter

Create a tool to convert existing scripts to the new configuration format:

```json
{
  "migration_tool": {
    "input_format": "groovy_script",
    "output_format": "unified_framework_config",
    "analysis_steps": [
      "parse_script",
      "identify_decision_factors",
      "extract_gateway_lists",
      "detect_enforcement_settings",
      "generate_filter_configurations",
      "generate_output_processor_configuration"
    ],
    "validation": {
      "run_equivalence_test": true,
      "test_scenarios": 100,
      "max_acceptable_divergence": 0
    }
  }
}
```

### 3. Hybrid Execution Mode

Support a transitional phase where both systems can run in parallel:

```json
{
  "execution_mode": {
    "primary": "unified_framework",
    "secondary": "legacy_script",
    "validation": {
      "enabled": true,
      "sampling_rate": 0.1,
      "divergence_threshold": 0,
      "alert_on_divergence": true
    },
    "fallback_strategy": {
      "on_unified_error": "use_legacy",
      "on_both_error": "use_default_list"
    }
  }
}
```

## Benefits for PL_v1 Integration

1. **Improved Maintainability**: Convert ad-hoc scripts to structured configurations
2. **Better Testability**: Test script logic in isolation from other components
3. **Enhanced Performance**: Optimize script execution through pattern-based execution
4. **Easier Governance**: Versioning, approval, and audit trails for routing logic
5. **Gradual Evolution**: Start with direct script mapping and gradually enhance

## Technical Implementation Considerations

1. **Backward Compatibility**: Ensure existing scripts continue to work
2. **Performance Parity**: Maintain or improve current execution speed
3. **Error Handling**: Robust handling of script execution failures
4. **Monitoring**: Comprehensive metrics for script execution times and errors
5. **Security**: Sandboxed execution environment for scripts
