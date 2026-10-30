# Euclid Routing Engine

## Overview

The Euclid Routing Engine is a flexible, pluggable rule evaluation system within the Decision Engine that enables merchants to define and deploy custom routing algorithms. Named after the ancient mathematician, Euclid provides a powerful Domain-Specific Language (DSL) for expressing complex routing logic that can evaluate transaction parameters in real-time to make optimal gateway selection decisions.

## Key Features

1. **Flexible Domain-Specific Language (DSL)**: A specialized language for defining routing logic
2. **Dynamic Rule Evaluation**: Real-time evaluation of transaction parameters against routing rules
3. **Multiple Output Types**: Support for different routing strategies (priority, percentage split, etc.)
4. **API-Driven Management**: APIs for creating, updating, and evaluating routing algorithms
5. **Versioning and Activation**: Support for multiple algorithm versions with active/inactive status

## Architecture

The Euclid Engine consists of several core components:

### Abstract Syntax Tree (AST)

Defined in `src/euclid/ast.rs`, the AST provides the structural representation of routing rules, including:

- **Conditions**: Expressions that evaluate to true/false
- **Statements**: Logical groupings of conditions
- **Rules**: Named collections of statements with associated outputs
- **Algorithm**: The complete set of rules, globals, and default selections

### Computation Graph (CGraph)

Implemented in `src/euclid/cgraph.rs`, the computation graph executes the routing algorithm by:

1. Converting the AST into an executable graph
2. Processing input parameters through the graph
3. Evaluating conditions and rules
4. Determining the appropriate output

### Interpreter

The interpreter (`src/euclid/interpreter.rs`) bridges between the API requests and the computation engine, handling:

- Parameter validation
- AST construction
- Computation graph execution
- Result formatting

## Rule Definition

### Algorithm Structure

A Euclid routing algorithm consists of:

1. **Globals**: Shared variables accessible throughout the algorithm
2. **Default Selection**: Fallback routing decision when no rules match
3. **Rules**: Named sets of conditions and outputs
4. **Metadata**: Additional information about the algorithm

### Rule Components

Each rule contains:

1. **Name**: Identifier for the rule
2. **Statements**: Logical conditions that determine if the rule applies
3. **Output**: The routing decision if the rule matches
4. **Routing Type**: The type of routing strategy (e.g., priority, percentage)

### Condition Format

Conditions follow this structure:

```json
{
  "lhs": "payment_method",
  "comparison": "equal",
  "value": {
    "type": "enum_variant",
    "value": "card"
  },
  "metadata": {}
}
```

Where:
- **lhs**: The left-hand side parameter to evaluate (e.g., payment_method, amount)
- **comparison**: The comparison operation (equal, greater_than, less_than, etc.)
- **value**: The right-hand side value with its type specification
- **metadata**: Additional information about the condition

## API Interface

Euclid provides several REST endpoints for managing and using routing algorithms:

### Create Routing Algorithm

```
POST /routing/create
```

Creates a new routing algorithm with the specified rules and configuration.

### Activate Routing Algorithm

```
POST /routing/activate
```

Sets a specific routing algorithm as active for a merchant.

### Evaluate Payment Parameters

```
POST /routing/evaluate
```

Evaluates a set of payment parameters against the active routing algorithm.

### List Routing Algorithms

```
POST /routing/list/{created_by}
```

Lists all routing algorithms for a specific merchant.

### List Active Routing Algorithm

```
POST /routing/list/active/{created_by}
```

Returns the currently active routing algorithm for a merchant.

## Example Algorithm

```json
{
  "name": "Priority Based Config",
  "created_by": "merchant_1",
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
}
```

## Routing Types

Euclid supports multiple routing types:

1. **Priority**: Ordered list of gateways to try in sequence
   ```json
   { "priority": ["gateway_a", "gateway_b", "gateway_c"] }
   ```

2. **Percentage Split**: Distribution of transactions across gateways by percentage
   ```json
   { "percentageSplit": { "gateway_a": 70, "gateway_b": 30 } }
   ```

3. **Single**: Selection of a specific gateway
   ```json
   { "single": "gateway_a" }
   ```

## Supported Parameter Types

Euclid can evaluate various parameter types:

1. **String**: Text values
   ```json
   { "type": "string", "value": "example" }
   ```

2. **Number**: Numeric values
   ```json
   { "type": "number", "value": 1000 }
   ```

3. **Boolean**: True/false values
   ```json
   { "type": "boolean", "value": true }
   ```

4. **Enum Variant**: Named variants for categorized values
   ```json
   { "type": "enum_variant", "value": "card" }
   ```

## Comparison Operations

The following comparison operations are supported:

1. **equal**: Exact equality matching
2. **not_equal**: Inequality matching
3. **greater_than**: Greater than comparison for numbers
4. **less_than**: Less than comparison for numbers
5. **greater_than_or_equal**: Greater than or equal comparison
6. **less_than_or_equal**: Less than or equal comparison
7. **in_list**: Checking if a value is in a list
8. **not_in_list**: Checking if a value is not in a list
9. **contains**: String contains substring check
10. **starts_with**: String starts with prefix check
11. **ends_with**: String ends with suffix check

## Use Cases

Euclid is particularly valuable for:

1. **Complex Routing Scenarios**: When routing decisions depend on multiple factors
2. **Dynamic Volume Splitting**: When transaction volume needs to be distributed across gateways
3. **Fine-grained Control**: When merchants need precise control over routing decisions
4. **Specialized Gateway Selection**: When certain transactions require specific gateway capabilities

## Integration with Decision Engine

Euclid integrates with the broader Decision Engine through:

1. **Rule Activation**: Active rules are considered in the decision-making process
2. **Parameter Mapping**: Transaction parameters are mapped to Euclid parameters
3. **Output Integration**: Euclid's output influences the final gateway selection

## Best Practices

1. **Start Simple**: Begin with basic rules and add complexity incrementally
2. **Test Thoroughly**: Verify rules with various transaction scenarios
3. **Use Clear Names**: Choose descriptive names for rules and conditions
4. **Document Logic**: Document the business logic behind routing decisions
5. **Consider Performance**: Complex rules may add processing overhead
6. **Version Control**: Keep track of algorithm versions and changes
7. **Monitor Effectiveness**: Regularly assess if routing decisions are achieving business goals
