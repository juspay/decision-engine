### 🚀 Feature: `gateway_id` for Gateways

This feature enables explicit differentiation between multiple instances of the same gateway (e.g., Stripe) in a profile, allowing proper routing and reporting.

---

#### 🧾 **Before (`gateway_id` not available)**

```text
               |----- Stripe ------|
profile ---                             ---> No way of differentiation  
               |----- Stripe ------|
```

- Multiple Stripe connectors used within a profile.
- No way to distinguish between them in routing or logs.
- Results in ambiguity during rule evaluation or payment tracking.

---

#### ✅ **Now (with `gateway_id`)**

```text
               |----- Stripe ------> gateway_1 ---> payment_1
profile ---                                                           ---> Proper differentiation  
               |----- Stripe ------> gateway_2 ---> payment_2
```

- Each Stripe connector is tied to a unique `gateway_id` (`gateway_1`, `gateway_2`).
- Payments routed distinctly via `payment_1`, `payment_2`.

---

### 1️⃣ Enum Variant Array Condition

Allows routing based on multiple card networks (e.g., Visa, Mastercard):

```json
{
  "name": "AND OR rule example",
  "created_by": "merchant_31",
  "description": "priority rule which demonstrates AND and OR rule",
  "algorithm_for": "payment",
  "algorithm": {
    "type": "advanced",
    "data": {
      "globals": {},
      "default_selection": {
        "priority": [
          {
            "gateway_name": "Bambora",
            "gateway_id": "mca_111"
          }
        ]
      },
      "rules": [
        {
          "name": "Card Rule",
          "routing_type": "priority",
          "output": {
            "priority": [
              {
                "gateway_name": "rbl",
                "gateway_id": "mca_114"
              }
            ]
          },
          "statements": [
            {
              "condition": [
                {
                  "lhs": "card_network",
                  "comparison": "equal",
                  "value": {
                    "type": "enum_variant_array",
                    "value": ["Visa", "Mastercard"]
                  },
                  "metadata": {}
                }
              ]
            }
          ]
        }
      ]
    }
  }
}
```

---

### 2️⃣ Number Array Condition

Matches exact amount values (e.g., 1000, 2000, 5000):

```json
{
  "name": "AND OR rule example",
  "created_by": "merchant_31",
  "description": "priority rule which demonstrates AND and OR rule",
  "algorithm_for": "payment",
  "algorithm": {
    "type": "advanced",
    "data": {
      "globals": {},
      "default_selection": {
        "priority": [
          {
            "gateway_name": "Bambora",
            "gateway_id": "mca_111"
          }
        ]
      },
      "rules": [
        {
          "name": "Card Rule",
          "routing_type": "priority",
          "output": {
            "priority": [
              {
                "gateway_name": "rbl",
                "gateway_id": "mca_114"
              }
            ]
          },
          "statements": [
            {
              "condition": [
                {
                  "lhs": "card_bin",
                  "comparison": "equal",
                  "value": {
                    "type": "number_array",
                    "value": [464646, 323232, 444444]
                  },
                  "metadata": {}
                }
              ]
            }
          ]
        }
      ]
    }
  }
}
```

---

### 3️⃣ Number Comparison Array + Additional String Match

Combines range-based amount filtering:

```json
{
  "name": "AND OR rule example",
  "created_by": "merchant_31",
  "description": "priority rule which demonstrates AND and OR rule",
  "algorithm": {
    "type": "advanced",
    "data": {
      "globals": {},
      "default_selection": {
        "priority": [
          {
            "gateway_name": "Bambora",
            "gateway_id": "mca_111"
          }
        ]
      },
      "rules": [
        {
          "name": "Card Rule",
          "routing_type": "priority",
          "output": {
            "priority": [
              {
                "gateway_name": "rbl",
                "gateway_id": "mca_114"
              }
            ]
          },
          "statements": [
            {
              "condition": [
                {
                  "lhs": "amount",
                  "comparison": "equal",
                  "value": {
                    "type": "number_comparison_array",
                    "value": [
                      {
                        "comparison_type": "greater_than",
                        "number": 1000
                      },
                      {
                        "comparison_type": "less_than_equal",
                        "number": 5000
                      }
                    ]
                  },
                  "metadata": {}
                }
              ]
            }
          ]
        }
      ]
    }
  }
}
```

---

## 🌟 Condition-Free Routing Features

Below are three rule types that **require no conditions**; they route payments (or payouts) {This is new as well, basically transaction segregator (you can have two rules active at once, one for payments, one for payouts and we can support more transaction kinds as well, so basically nof transation_types == nof of active rules at once )} directly based on their configuration.

---

### 1️⃣ Volume-Split Rule

Splits traffic between connectors by percentage.

```bash
curl --location 'http://localhost:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '{
  "name": "volume split test rule",
  "description": "A rule to split volume between connectors",
  "created_by": "merchant_31",
  "algorithm_for": "payout",
  "algorithm": {
    "type": "volume_split",
    "data": [
      {
        "split": 70,
        "output": {
          "gateway_name": "stripe",
          "gateway_id": "mca_001"
        }
      },
      {
        "split": 30,
        "output": {
          "gateway_name": "razorpay",
          "gateway_id": "mca_002"
        }
      }
    ]
  }
}'
```

---

### 2️⃣ Priority Rule

Attempts connectors in the specified order until one succeeds.

```bash
curl --location 'http://localhost:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '{
  "name": "priority rule test",
  "description": "Prioritize connectors by order",
  "created_by": "merchant_123",
  "algorithm": {
    "type": "priority",
    "data": [
      {
        "gateway_name": "stripe",
        "gateway_id": "mca_001"
      },
      {
        "gateway_name": "razorpay",
        "gateway_id": "mca_002"
      }
    ]
  }
}'
```

---

### 3️⃣ Single Connector Rule

Always routes to one fixed connector.

```bash
curl --location 'http://localhost:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '{
  "name": "single connector rule",
  "description": "Only one connector to use",
  "created_by": "merchant_123",
  "algorithm": {
    "type": "single",
    "data": {
      "gateway_name": "stripe",
      "gateway_id": "mca_00123"
    }
  }
}'
```
---
