# Priority Logic Routing

## Overview

Priority Logic Routing is a rule-based routing approach in the Decision Engine that determines the sequence in which payment gateways should be tried based on predefined merchant rules. This approach ensures predictable and obligation-driven payment processing by enforcing business rules that may include contractual obligations, volume commitments, or strategic preferences.

## How It Works

The Priority Logic Router evaluates a set of conditions against the current transaction details and determines a priority list of gateways. This priority list represents the order in which payment gateways should be attempted for the transaction.

### Key Components

1. **Rule Configuration**: A Groovy-based script that defines the rules for determining gateway priorities
2. **Rule Evaluation Engine**: A component that interprets the rules and evaluates them against transaction details
3. **Priority Determination**: The process of creating an ordered list of gateways based on rule evaluation
4. **Enforcement Flag**: An optional flag that can strictly enforce the use of specified gateways

## Configuration

Priority Logic is configured through a Groovy script file (`priority_logic.txt`) that is loaded into the database. This script contains the rules that determine gateway priorities for different transaction scenarios.

### Example Configuration

```groovy
def priorities = ['A','B','C','D','E'] // Default priorities if no rule matches
def systemtimemills = System.currentTimeMillis() % 100
def enforceFlag = false

if ((payment.paymentMethodType)=='UPI' && (txn.sourceObject)=='UPI_COLLECT') {
    priorities = ['A','B']
    enforceFlag = true
}
else {
    if (['UPI'].contains(payment.paymentMethodType)) {
        if (order.udf1=="LOB1") {
            if (payment.paymentSource?.indexOf("ABC") >= 0 || 
                payment.paymentSource?.indexOf("DEF") >= 0) {
                priorities = ['B','C']
            }
            else if (systemtimemills < 50) {
                priorities = ['D','E']
            }
            else {
                priorities = ['E','D']
            }
        }
    }
}
```

In this example:
- Default gateway priority is A → B → C → D → E
- For UPI_COLLECT transactions, it enforces A → B with strict enforcement
- For other UPI transactions with specific conditions, it sets different priority orders

### Configuration Structure

The configuration script must define:
1. `priorities`: An array of gateway identifiers in priority order
2. (Optional) `enforceFlag`: A boolean that, when true, strictly enforces the use of only the gateways in the priorities list

## Rule Evaluation Process

1. The transaction details (payment method, amount, currency, etc.) are passed to the rule engine
2. The engine evaluates the conditions in the script using these details
3. Based on which conditions are met, it sets the appropriate priorities array
4. The resulting priorities are returned to the Decision Engine

## Integration with Other Components

Priority Logic Routing integrates with other components of the Decision Engine:

1. **Success Rate Routing**: Priority Logic can provide the initial ordering, which Success Rate Routing can then refine
2. **Elimination Logic**: Gateways identified as experiencing issues can be filtered out from the priority list
3. **Feedback Loop**: Transaction outcomes don't directly affect Priority Logic (unlike Success Rate Routing)

## Use Cases

Priority Logic Routing is particularly valuable for:

1. **Contractual Obligations**: When merchants have commitments to process specific volumes through certain gateways
2. **Business Rules Enforcement**: When specific transaction types must follow predetermined paths
3. **A/B Testing**: By using time-based conditions to route similar transactions to different gateways
4. **Gateway Specialization**: When certain gateways are preferred for specific payment methods or transaction types
5. **Regulatory Compliance**: When transactions must follow specific routing rules for compliance reasons

## Limitations

1. **Static Rules**: Priority Logic rules are relatively static and require manual updates
2. **No Automatic Optimization**: Unlike Success Rate Routing, Priority Logic doesn't automatically optimize based on performance
3. **Complexity Management**: Complex rules can become difficult to maintain and understand
4. **Performance Overhead**: Complex rule evaluation can add processing time

## Best Practices

1. **Keep Rules Simple**: Start with simple rules and add complexity only as needed
2. **Test Thoroughly**: Verify that rules work as expected for all transaction scenarios
3. **Document Rules**: Maintain clear documentation of the business logic behind routing rules
4. **Regular Review**: Periodically review and update rules to ensure they remain aligned with business needs
5. **Use Comments**: Add comments in the script to explain the rationale for specific rules
