#!/bin/bash

# Set the API base URL
API_BASE="http://127.0.0.1:8080"
PRIORITY_ID=""
VOLUME_SPLIT_ID=""
VOLUME_SPLIT_PRIORITY_ID=""
NESTED_RULE_ID=""

# Output markdown file
OUTPUT_FILE="routing_test_results.md"

# Initialize the markdown file
echo "# Routing Rules Test Results" > "$OUTPUT_FILE"
echo "Generated on: $(date)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Colors for better readability
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to extract routing ID from response
extract_routing_id() {
    local response=$1
    echo "$response" | grep -o '"rule_id":"[^"]*"' | cut -d ':' -f2 | tr -d '"'
}

# Function to add section to markdown file
add_to_markdown() {
    local title=$1
    local content=$2

    echo "## $title" >> "$OUTPUT_FILE"
    echo "$content" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
}

# Function to add test result to markdown file
add_test_result() {
    local test_name=$1
    local parameters=$2
    local expected=$3
    local response=$4

    echo "### $test_name" >> "$OUTPUT_FILE"
    echo "**Parameters:** $parameters" >> "$OUTPUT_FILE"
    echo "**Expected:** $expected" >> "$OUTPUT_FILE"
    echo "**Response:**" >> "$OUTPUT_FILE"
    echo '```json' >> "$OUTPUT_FILE"
    echo "$response" | jq >> "$OUTPUT_FILE"
    echo '```' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
}

echo -e "${BLUE}===========================================================${NC}"
echo -e "${BLUE}                 Routing Rules Test Script                 ${NC}"
echo -e "${BLUE}===========================================================${NC}"
echo -e "${BLUE}Results will be saved to: $OUTPUT_FILE${NC}"

# ============= CREATION OF ROUTING RULES =============
echo -e "\n${YELLOW}CREATING ROUTING RULES...${NC}\n"

# 1. Create a Priority-based routing rule
echo -e "${GREEN}1. Creating priority-based routing rule...${NC}"
PRIORITY_RESPONSE=$(curl -s -X POST \
  "${API_BASE}/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Priority Based Config",
    "algorithm": {
        "globals": {},
        "defaultSelection": {
            "priority": ["stripe", "adyen", "checkout"]
        },
        "rules": [
            {
                "name": "Card Rule",
                "routingType": "priority",
                "output": {
                    "priority": ["stripe", "adyen"]
                },
                "statements": [
                    {
                        "condition": [
                            {
                                "lhs": "payment_method",
                                "comparison": "equal",
                                "value": {
                                    "type": "enum_variant",
                                    "value": "card"
                                },
                                "metadata": {}
                            },
                            {
                                "lhs": "amount",
                                "comparison": "greater_than",
                                "value": {
                                    "type": "number",
                                    "value": 1000
                                },
                                "metadata": {}
                            }
                        ]
                    }
                ]
            }
        ],
        "metadata": {}
    }
}')

echo "$PRIORITY_RESPONSE"
add_to_markdown "Priority Rule Creation" "$(echo "$PRIORITY_RESPONSE" | jq)"
PRIORITY_ID=$(extract_routing_id "$PRIORITY_RESPONSE")
if [ -n "$PRIORITY_ID" ]; then
    echo -e "${GREEN}Priority rule created with ID: $PRIORITY_ID${NC}\n"
else
    echo -e "${RED}Failed to create priority rule or extract ID${NC}\n"
fi

# 2. Create a Volume Split routing rule
echo -e "${GREEN}2. Creating volume split routing rule...${NC}"
VOLUME_SPLIT_RESPONSE=$(curl -s -X POST \
  "${API_BASE}/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Volume Split Config",
    "algorithm": {
        "globals": {},
        "defaultSelection": {
            "priority": ["stripe", "adyen", "checkout"]
        },
        "rules": [
            {
                "name": "Card Volume Split",
                "routingType": "volume_split",
                "output": {
                    "volumeSplit": [
                        {
                            "split": 60,
                            "output": "stripe"
                        },
                        {
                            "split": 40,
                            "output": "adyen"
                        }
                    ]
                },
                "statements": [
                    {
                        "condition": [
                            {
                                "lhs": "payment_method",
                                "comparison": "equal",
                                "value": {
                                    "type": "enum_variant",
                                    "value": "card"
                                },
                                "metadata": {}
                            }
                        ]
                    }
                ]
            }
        ],
        "metadata": {}
    }
}')

echo "$VOLUME_SPLIT_RESPONSE"
add_to_markdown "Volume Split Rule Creation" "$(echo "$VOLUME_SPLIT_RESPONSE" | jq)"
VOLUME_SPLIT_ID=$(extract_routing_id "$VOLUME_SPLIT_RESPONSE")
if [ -n "$VOLUME_SPLIT_ID" ]; then
    echo -e "${GREEN}Volume split rule created with ID: $VOLUME_SPLIT_ID${NC}\n"
else
    echo -e "${RED}Failed to create volume split rule or extract ID${NC}\n"
fi

# 3. Create a Volume Split Priority routing rule
echo -e "${GREEN}3. Creating volume split priority routing rule...${NC}"
VOLUME_SPLIT_PRIORITY_RESPONSE=$(curl -s -X POST \
  "${API_BASE}/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Volume Split Priority Config",
    "algorithm": {
        "globals": {},
        "defaultSelection": {
            "priority": ["stripe", "adyen", "checkout"]
        },
        "rules": [
            {
                "name": "Card VSP Rule",
                "routingType": "volume_split_priority",
                "output": {
                    "volumeSplitPriority": [
                        {
                            "split": 50,
                            "output": ["stripe", "adyen"]
                        },
                        {
                            "split": 30,
                            "output": ["checkout", "bambora"]
                        },
                        {
                            "split": 20,
                            "output": ["adyen", "stripe", "checkout"]
                        }
                    ]
                },
                "statements": [
                    {
                        "condition": [
                            {
                                "lhs": "payment_method",
                                "comparison": "equal",
                                "value": {
                                    "type": "enum_variant",
                                    "value": "card"
                                },
                                "metadata": {}
                            }
                        ]
                    }
                ]
            }
        ],
        "metadata": {}
    }
}')

echo "$VOLUME_SPLIT_PRIORITY_RESPONSE"
add_to_markdown "Volume Split Priority Rule Creation" "$(echo "$VOLUME_SPLIT_PRIORITY_RESPONSE" | jq)"
VOLUME_SPLIT_PRIORITY_ID=$(extract_routing_id "$VOLUME_SPLIT_PRIORITY_RESPONSE")
if [ -n "$VOLUME_SPLIT_PRIORITY_ID" ]; then
    echo -e "${GREEN}Volume split priority rule created with ID: $VOLUME_SPLIT_PRIORITY_ID${NC}\n"
else
    echo -e "${RED}Failed to create volume split priority rule or extract ID${NC}\n"
fi

# 4. Create a rule with nested conditions
echo -e "${GREEN}4. Creating rule with nested conditions...${NC}"
NESTED_RULE_RESPONSE=$(curl -s -X POST \
  "${API_BASE}/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Nested Condition Rule",
    "algorithm": {
        "globals": {},
        "defaultSelection": {
            "priority": ["stripe", "adyen", "checkout"]
        },
        "rules": [
            {
                "name": "High Value Card Rule",
                "routingType": "priority",
                "output": {
                    "priority": ["adyen", "stripe"]
                },
                "statements": [
                    {
                        "condition": [
                            {
                                "lhs": "payment_method",
                                "comparison": "equal",
                                "value": {
                                    "type": "enum_variant",
                                    "value": "card"
                                },
                                "metadata": {}
                            }
                        ],
                        "nested": [
                            {
                                "condition": [
                                    {
                                        "lhs": "amount",
                                        "comparison": "greater_than",
                                        "value": {
                                            "type": "number",
                                            "value": 5000
                                        },
                                        "metadata": {}
                                    },
                                    {
                                        "lhs": "currency",
                                        "comparison": "equal",
                                        "value": {
                                            "type": "enum_variant",
                                            "value": "USD"
                                        },
                                        "metadata": {}
                                    }
                                ]
                            },
                            {
                                "condition": [
                                    {
                                        "lhs": "amount",
                                        "comparison": "greater_than",
                                        "value": {
                                            "type": "number",
                                            "value": 10000
                                        },
                                        "metadata": {}
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        ],
        "metadata": {}
    }
}')

echo "$NESTED_RULE_RESPONSE"
add_to_markdown "Nested Rule Creation" "$(echo "$NESTED_RULE_RESPONSE" | jq)"
NESTED_RULE_ID=$(extract_routing_id "$NESTED_RULE_RESPONSE")

if [ -n "$NESTED_RULE_ID" ]; then
    echo -e "${GREEN}Nested rule created with ID: $NESTED_RULE_ID${NC}\n"
else
    echo -e "${RED}Failed to create nested rule or extract ID${NC}\n"
fi

# 5. Create a complex rule with global variables
echo -e "${GREEN}5. Creating complex rule with global references...${NC}"
COMPLEX_RULE_RESPONSE=$(curl -s -X POST \
  "${API_BASE}/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Complex Routing Rule",
    "algorithm": {
      "globals": {
        "visa_train_bins": [
          {"type": "str_value", "value": "401063"},
          {"type": "str_value", "value": "418212"},
          {"type": "str_value", "value": "402275"},
          {"type": "str_value", "value": "421389"},
          {"type": "str_value", "value": "471227"},
          {"type": "str_value", "value": "523435"}
        ],
        "travel_verticals": [
          {"type": "str_value", "value": "TRAIN"},
          {"type": "str_value", "value": "FLIGHT"},
          {"type": "str_value", "value": "BUS"},
          {"type": "str_value", "value": "ABUS"},
          {"type": "str_value", "value": "HOTEL"}
        ]
      },
      "defaultSelection": {
        "priority": ["RAZORPAY", "PAYU"]
      },
      "rules": [
        {
          "name": "Special BIN Travel Rule",
          "routingType": "priority",
          "output": {
            "priority": ["PAYU"]
          },
          "statements": [
            {
              "condition": [
                {
                  "lhs": "payment_cardBin",
                  "comparison": "equal",
                  "value": {
                    "type": "global_ref",
                    "value": "visa_train_bins"
                  },
                  "metadata": {}
                }
              ],
              "nested": [
                {
                  "condition": [
                    {
                      "lhs": "order_udf1",
                      "comparison": "equal",
                      "value": {
                        "type": "global_ref",
                        "value": "travel_verticals"
                      },
                      "metadata": {}
                    }
                  ]
                }
              ]
            }
          ]
        }
      ],
      "metadata": {}
    }
  }'
)

echo "$COMPLEX_RULE_RESPONSE"
add_to_markdown "Complex Rule Creation" "$(echo "$COMPLEX_RULE_RESPONSE" | jq)"
COMPLEX_RULE_ID=$(extract_routing_id "$COMPLEX_RULE_RESPONSE")
if [ -n "$COMPLEX_RULE_ID" ]; then
    echo -e "${GREEN}Complex rule created with ID: $COMPLEX_RULE_ID${NC}\n"
else
    echo -e "${RED}Failed to create complex rule or extract ID${NC}\n"
fi

sleep 2  # Give the server some time to process

# Add rule IDs to markdown
echo "## Created Rules" >> "$OUTPUT_FILE"
# echo "- Priority Rule ID: \`$PRIORITY_ID\`" >> "$OUTPUT_FILE"
# echo "- Volume Split Rule ID: \`$VOLUME_SPLIT_ID\`" >> "$OUTPUT_FILE"
# echo "- Volume Split Priority Rule ID: \`$VOLUME_SPLIT_PRIORITY_ID\`" >> "$OUTPUT_FILE"
echo "- Nested Rule ID: \`$NESTED_RULE_ID\`" >> "$OUTPUT_FILE"
# echo "- Complex Rule ID: \`$COMPLEX_RULE_ID\`" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "## Test Results" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# ============= TESTING ROUTING RULES =============
echo -e "\n${YELLOW}TESTING ROUTING RULES...${NC}\n"

# Test Priority Routing - Match Card Rule
if [ -n "$PRIORITY_ID" ]; then
    echo -e "${GREEN}1. Testing priority routing - Card Rule Match...${NC}"
    TEST_PARAMS="payment_method=card, amount=2000"
    TEST_EXPECTED="Should match 'Card Rule' and return [stripe, adyen]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$PRIORITY_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 2000
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Priority - Card Rule Match" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Priority Routing - Default Selection (No Match)
if [ -n "$PRIORITY_ID" ]; then
    echo -e "${GREEN}2. Testing priority routing - Default Selection (insufficient amount)...${NC}"
    TEST_PARAMS="payment_method=card, amount=500"
    TEST_EXPECTED="Should use default selection [stripe, adyen, checkout] (amount < 1000)"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$PRIORITY_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 500
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Priority - Default Selection (insufficient amount)" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Priority Routing - Default Selection (Different payment method)
if [ -n "$PRIORITY_ID" ]; then
    echo -e "${GREEN}3. Testing priority routing - Default Selection (different payment method)...${NC}"
    TEST_PARAMS="payment_method=bank_debit, amount=2000"
    TEST_EXPECTED="Should use default selection [stripe, adyen, checkout] (payment_method doesn't match)"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$PRIORITY_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"bank_debit\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 2000
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Priority - Default Selection (different payment method)" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Volume Split Routing
if [ -n "$VOLUME_SPLIT_ID" ]; then
    echo -e "${GREEN}4. Testing volume split routing - Should randomly select based on percentages...${NC}"
    TEST_PARAMS="payment_method=card, amount=1500"
    TEST_EXPECTED="Should select either 'stripe' (60%) or 'adyen' (40%) randomly"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}" 
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$VOLUME_SPLIT_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 1500
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Volume Split - Random Selection 1" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
    
    # Run it again to show randomness
    echo -e "${GREEN}5. Testing volume split routing again - May select different connector...${NC}"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$VOLUME_SPLIT_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 1500
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Volume Split - Random Selection 2" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Volume Split Priority Routing
if [ -n "$VOLUME_SPLIT_PRIORITY_ID" ]; then
    echo -e "${GREEN}6. Testing volume split priority routing - Should randomly select a priority list...${NC}"
    TEST_PARAMS="payment_method=card, amount=1500"
    TEST_EXPECTED="Should select one of: 50%: [stripe, adyen], 30%: [checkout, bambora], 20%: [adyen, stripe, checkout]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$VOLUME_SPLIT_PRIORITY_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 1500
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Volume Split Priority - Random Selection 1" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
    
    # Run it again to show randomness
    echo -e "${GREEN}7. Testing volume split priority routing again - May select different list...${NC}"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$VOLUME_SPLIT_PRIORITY_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 1500
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Volume Split Priority - Random Selection 2" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Volume Split Routing with different payment method (default selection)
if [ -n "$VOLUME_SPLIT_ID" ]; then
    echo -e "${GREEN}8. Testing volume split routing with different payment method (default selection)...${NC}"
    TEST_PARAMS="payment_method=bank_debit, amount=1500"
    TEST_EXPECTED="Should use default selection [stripe, adyen, checkout] (payment_method doesn't match)"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$VOLUME_SPLIT_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"bank_debit\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 1500
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Volume Split - Default Selection" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Nested Rule - Matching First Nested Condition
if [ -n "$NESTED_RULE_ID" ]; then
    echo -e "${GREEN}9. Testing nested rule - First Nested Condition Match...${NC}"
    TEST_PARAMS="payment_method=card, amount=6000, currency=USD"
    TEST_EXPECTED="Should match 'High Value Card Rule' via first nested condition (amount > 5000 and currency = USD), returning [adyen, stripe]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$NESTED_RULE_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 6000
            },
            \"currency\": {
                \"type\": \"enum_variant\",
                \"value\": \"USD\"
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Nested Rule - First Nested Condition Match" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Nested Rule - Matching Second Nested Condition
if [ -n "$NESTED_RULE_ID" ]; then
    echo -e "${GREEN}10. Testing nested rule - Second Nested Condition Match...${NC}"
    TEST_PARAMS="payment_method=card, amount=12000, currency=EUR"
    TEST_EXPECTED="Should match 'High Value Card Rule' via second nested condition (amount > 10000, any currency), returning [adyen, stripe]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$NESTED_RULE_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 12000
            },
            \"currency\": {
                \"type\": \"enum_variant\",
                \"value\": \"EUR\"
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Nested Rule - Second Nested Condition Match" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Test Nested Rule - No Match
if [ -n "$NESTED_RULE_ID" ]; then
    echo -e "${GREEN}11. Testing nested rule - No Match...${NC}"
    TEST_PARAMS="payment_method=card, amount=3000, currency=USD"
    TEST_EXPECTED="Should not match any condition (amount < 5000), returning default [stripe, adyen, checkout]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    RESPONSE=$(curl -s -X POST \
    "${API_BASE}/routing/evaluate" \
    -H "Content-Type: application/json" \
    -d "{
        \"routing_id\": \"$NESTED_RULE_ID\",
        \"parameters\": {
            \"payment_method\": {
                \"type\": \"enum_variant\",
                \"value\": \"card\"
            },
            \"amount\": {
                \"type\": \"number\",
                \"value\": 3000
            },
            \"currency\": {
                \"type\": \"enum_variant\",
                \"value\": \"USD\"
            }
        }
    }")
    
    echo "$RESPONSE" | jq
    add_test_result "Nested Rule - No Match" "$TEST_PARAMS" "$TEST_EXPECTED" "$RESPONSE"
    echo -e "\n"
fi

# Get the complex rule ID first (assuming it's stored in COMPLEX_RULE_ID variable)
if [ -n "$COMPLEX_RULE_ID" ]; then
    # Test case where both conditions match
    echo -e "${GREEN}Testing complex rule with global references...${NC}"
    TEST_PARAMS="payment.cardBin=401063, order.udf1=TRAIN"
    TEST_EXPECTED="Should match 'Special BIN Travel Rule', returning [PAYU]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    TEST_COMPLEX_RULE_RESPONSE=$(curl -s -X POST \
      "${API_BASE}/routing/evaluate" \
      -H "Content-Type: application/json" \
      -d "{
        \"routing_id\": \"$COMPLEX_RULE_ID\",
        \"parameters\": {
            \"payment_cardBin\": {
                \"type\": \"str_value\",
                \"value\": \"401063\"
            },
            \"order_udf1\": {
                \"type\": \"str_value\",
                \"value\": \"TRAIN\"
            }
        }
      }")
    
    echo "$TEST_COMPLEX_RULE_RESPONSE" | jq
    add_test_result "Complex Rule - Full Match" "$TEST_PARAMS" "$TEST_EXPECTED" "$TEST_COMPLEX_RULE_RESPONSE"
    echo -e "\n"

    # Test case where cardBin matches but udf1 doesn't
    echo -e "${GREEN}Testing case where cardBin matches but udf1 doesn't...${NC}"
    TEST_PARAMS="payment.cardBin=401063, order.udf1=CAR_RENTAL"
    TEST_EXPECTED="Should not match 'Special BIN Travel Rule', returning default [RAZORPAY, PAYU]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    TEST_PARTIAL_MATCH=$(curl -s -X POST \
      "${API_BASE}/routing/evaluate" \
      -H "Content-Type: application/json" \
      -d "{
        \"routing_id\": \"$COMPLEX_RULE_ID\",
        \"parameters\": {
            \"payment_cardBin\": {
                \"type\": \"str_value\",
                \"value\": \"401063\"
            },
            \"order_udf1\": {
                \"type\": \"str_value\",
                \"value\": \"CAR_RENTAL\"
            }
        }
      }")
    
    echo "$TEST_PARTIAL_MATCH" | jq
    add_test_result "Complex Rule - Partial Match" "$TEST_PARAMS" "$TEST_EXPECTED" "$TEST_PARTIAL_MATCH"
    echo -e "\n"

    # Test case where nothing matches (should fall back to default)
    echo -e "${GREEN}Testing case where nothing matches...${NC}"
    TEST_PARAMS="payment.cardBin=123456, order.udf1=CAR_RENTAL"
    TEST_EXPECTED="Should not match any rule, returning default [RAZORPAY, PAYU]"
    echo -e "${BLUE}Parameters: $TEST_PARAMS${NC}"
    echo -e "${BLUE}Expected: $TEST_EXPECTED${NC}"
    
    TEST_NO_MATCH=$(curl -s -X POST \
      "${API_BASE}/routing/evaluate" \
      -H "Content-Type: application/json" \
      -d "{
        \"routing_id\": \"$COMPLEX_RULE_ID\",
        \"parameters\": {
            \"payment_cardBin\": {
                \"type\": \"str_value\",
                \"value\": \"123456\"
            },
            \"order_udf1\": {
                \"type\": \"str_value\",
                \"value\": \"CAR_RENTAL\"
            }
        }
      }")
    
    echo "$TEST_NO_MATCH" | jq
    add_test_result "Complex Rule - No Match" "$TEST_PARAMS" "$TEST_EXPECTED" "$TEST_NO_MATCH"
    echo -e "\n"
fi

echo -e "${BLUE}===========================================================${NC}"
echo -e "${BLUE}                 Testing Complete                          ${NC}"
echo -e "${BLUE}Results saved to: ${GREEN}$OUTPUT_FILE${NC}"
echo -e "${BLUE}===========================================================${NC}"
