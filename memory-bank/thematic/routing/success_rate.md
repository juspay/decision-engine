# Success Rate Based Routing

## Overview

Success Rate Based Routing is a data-driven routing strategy in the Decision Engine that optimizes payment gateway selection based on historical transaction success metrics. Unlike rule-based approaches, Success Rate Routing automatically adapts to changing conditions by analyzing past performance, enabling merchants to maximize transaction success rates without manual intervention.

## How It Works

The Success Rate Router analyzes historical transaction data to determine which gateways have the highest success rates for specific transaction profiles. The system then routes new transactions to the gateways most likely to succeed based on similar transaction characteristics.

### Key Components

1. **Transaction Data Collection**: Collection of transaction outcomes (success/failure) from the feedback loop
2. **Success Rate Calculation**: Computation of success rates for gateways across various dimensions
3. **Scoring Algorithm**: System for ranking gateways based on calculated success rates
4. **Gateway Selection**: Process for selecting the optimal gateway based on scores
5. **Hedging Mechanism**: Approach for exploring alternative gateways to prevent local maxima

## Configuration

Success Rate Routing is configured through YAML configuration files that define parameters such as bucket sizes and hedging percentages.

### Example Configuration

```yaml
sr_routing_config:
  defaultBucketSize: 50
  defaultHedgingPercent: 5
  subLevelInputConfig:
    - paymentMethodType: UPI
      paymentMethod: UPI_COLLECT
      bucketSize: 100
      hedgingPercent: 1
    - paymentMethodType: UPI
      paymentMethod: UPI_PAY
      bucketSize: 500
      hedgingPercent: 1
    - paymentMethodType: UPI
      paymentMethod: UPI_QR
      bucketSize: 1000
      hedgingPercent: 1
    - paymentMethodType: NB
      bucketSize: 50
      hedgingPercent: 1
    - paymentMethodType: CARD
      bucketSize: 200
      hedgingPercent: 1
    - paymentMethodType: WALLET
      bucketSize: 50
      hedgingPercent: 1
```

### Configuration Parameters

1. **defaultBucketSize**: The default number of recent transactions to consider when calculating success rates
2. **defaultHedgingPercent**: The default percentage of transactions to route to non-optimal gateways for exploration
3. **subLevelInputConfig**: Specific configuration for different payment method types and methods
   - **paymentMethodType**: The type of payment method (e.g., UPI, CARD, NB)
   - **paymentMethod**: The specific payment method within the type (e.g., UPI_PAY, UPI_COLLECT)
   - **bucketSize**: The number of recent transactions to consider for this specific method
   - **hedgingPercent**: The percentage of transactions to route to non-optimal gateways for this method

## Success Rate Calculation Process

1. **Dimension Identification**: Identify the relevant dimension for the transaction (e.g., CARD + VISA + INR)
2. **Data Collection**: Collect the most recent N transactions for this dimension (where N is the bucket size)
3. **Success Rate Calculation**: Calculate the success rate for each gateway in this dimension
4. **Gateway Ranking**: Rank gateways based on their success rates
5. **Selection Decision**: Select the highest-ranked gateway, with a small percentage (hedging percent) going to other gateways

## Routing Dimensions

Success Rate Routing operates across multiple dimensions to provide granular optimization:

1. **Payment Method Type**: Card, UPI, Net Banking, Wallet, etc.
2. **Payment Method**: Specific variants within a type (e.g., UPI_PAY, UPI_COLLECT)
3. **Card Type**: Credit, Debit, Prepaid (for card transactions)
4. **Card Network**: Visa, MasterCard, Amex, etc. (for card transactions)
5. **Currency**: Transaction currency
6. **Amount Range**: Transaction amount brackets
7. **Issuer Bank**: The bank that issued the payment instrument

The system calculates success rates at various dimension combinations, allowing for precise routing decisions.

## Feedback Loop Integration

Success Rate Routing relies heavily on the feedback loop to maintain up-to-date performance metrics:

1. **Transaction Outcome Recording**: Each transaction outcome is recorded via the `/update-gateway-score` endpoint
2. **Success/Failure Classification**: Outcomes are classified as success or failure based on response codes
3. **Metric Updates**: Success rates are recalculated with each new transaction outcome
4. **Cache Refreshing**: Updated metrics are cached for quick access in future decisions

## Hedging Strategy

To prevent the system from getting stuck in local maxima and to continuously explore potential improvements, a hedging strategy is employed:

1. **Exploration Percentage**: A small percentage of transactions (hedging percent) are routed to non-optimal gateways
2. **Controlled Experimentation**: This allows the system to gather performance data on all gateways, not just the current best performer
3. **Adaptive Learning**: As performance data accumulates, the system may discover that previously lower-ranked gateways are now performing better

## Integration with Other Components

Success Rate Routing integrates with other components of the Decision Engine:

1. **Priority Logic**: Can use Priority Logic as a starting point for gateway selection
2. **Elimination Logic**: Underperforming gateways can be eliminated from consideration
3. **Feedback System**: Relies on transaction outcome data to optimize future decisions

## Performance Considerations

1. **Cache Usage**: Success rates are cached to minimize database queries
2. **Bucket Size Tuning**: Larger bucket sizes provide more stable metrics but slower adaptation to changes
3. **Hedging Percent Balancing**: Higher hedging percentages provide more exploration but may reduce overall success rates
4. **Dimension Granularity**: More granular dimensions provide more precise routing but require more data

## Use Cases

Success Rate Routing is particularly valuable for:

1. **High-Volume Merchants**: Where small improvements in success rates translate to significant revenue
2. **Dynamic Payment Environments**: Where gateway performance changes frequently
3. **Diverse Payment Methods**: Where different gateways excel at processing different payment types
4. **Automated Optimization**: When manual rule management is impractical

## Limitations

1. **Cold Start Problem**: New gateways or payment methods have limited historical data for decision-making
2. **Data Requirement**: Requires a sufficient volume of transactions to build reliable metrics
3. **Temporal Variations**: May not account for time-based performance variations (e.g., day vs. night performance)
4. **Feedback Delays**: Relies on timely and accurate feedback for optimization

## Best Practices

1. **Combine with Priority Logic**: Use Priority Logic for essential business rules and Success Rate for optimization
2. **Appropriate Bucket Sizes**: Configure bucket sizes based on transaction volumes - higher volumes allow larger buckets
3. **Regular Monitoring**: Monitor performance to ensure the system is adapting as expected
4. **Balanced Hedging**: Set hedging percentages low enough to maximize success but high enough to explore alternatives
5. **Feedback Timeliness**: Ensure transaction outcomes are reported promptly for accurate optimization
