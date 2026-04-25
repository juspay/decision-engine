const CONNECTORS = {
  stripe: { gateway_name: 'stripe', gateway_id: 'mca_stripe' },
  adyen: { gateway_name: 'adyen', gateway_id: 'mca_adyen' },
  checkout: { gateway_name: 'checkout', gateway_id: 'mca_checkout' },
  paytm: { gateway_name: 'paytm', gateway_id: 'mca_paytm' },
  razorpay: { gateway_name: 'razorpay', gateway_id: 'mca_razorpay' },
}

function uniqueSuffix() {
  return `${Date.now()}_${Math.floor(Math.random() * 100000)}`
}

function merchantId(suite = 'merchant') {
  return `cy_${suite}_${uniqueSuffix()}`
}

function paymentId(prefix = 'pay') {
  return `${prefix}_${uniqueSuffix()}`
}

function customerId(prefix = 'cust') {
  return `${prefix}_${uniqueSuffix()}`
}

function ruleName(prefix = 'rule') {
  return `${prefix}_${uniqueSuffix()}`
}

function gatewayConnector(name, gatewayId = null) {
  const connector = CONNECTORS[name]
  if (connector) {
    return {
      gateway_name: connector.gateway_name,
      gateway_id: gatewayId ?? connector.gateway_id,
    }
  }

  return {
    gateway_name: name,
    gateway_id: gatewayId,
  }
}

function connectorNames(...names) {
  return names.map((name) => gatewayConnector(name).gateway_name)
}

function srConfigData(overrides = {}) {
  return {
    defaultLatencyThreshold: 90,
    defaultSuccessRate: 0.5,
    defaultBucketSize: 200,
    defaultHedgingPercent: 5,
    subLevelInputConfig: [
      {
        paymentMethodType: 'upi',
        paymentMethod: 'upi_collect',
        bucketSize: 250,
        hedgingPercent: 1,
        latencyThreshold: null,
      },
    ],
    ...overrides,
  }
}

function eliminationConfigData(overrides = {}) {
  return {
    threshold: 0.35,
    txnLatency: {
      gatewayLatency: 5000,
    },
    ...overrides,
  }
}

function debitRoutingConfigData(overrides = {}) {
  return {
    merchant_category_code: '5411',
    acquirer_country: 'US',
    ...overrides,
  }
}

function paymentInfo(overrides = {}) {
  return {
    paymentId: paymentId(),
    amount: 100.5,
    currency: 'USD',
    customerId: customerId(),
    udfs: null,
    preferredGateway: null,
    paymentType: 'ORDER_PAYMENT',
    metadata: null,
    internalMetadata: null,
    isEmi: false,
    emiBank: null,
    emiTenure: null,
    paymentMethodType: 'UPI',
    paymentMethod: 'UPI_PAY',
    paymentSource: null,
    authType: null,
    cardIssuerBankName: null,
    cardIsin: null,
    cardType: null,
    cardSwitchProvider: null,
    ...overrides,
  }
}

function srDecideGatewayRequest(overrides = {}) {
  const paymentInfoOverrides = overrides.paymentInfo || {}

  return {
    merchantId: overrides.merchantId || merchantId('dynamic'),
    eligibleGatewayList:
      overrides.eligibleGatewayList || connectorNames('stripe', 'adyen', 'checkout'),
    rankingAlgorithm: overrides.rankingAlgorithm || 'SR_BASED_ROUTING',
    eliminationEnabled:
      overrides.eliminationEnabled === undefined ? true : overrides.eliminationEnabled,
    paymentInfo: paymentInfo(paymentInfoOverrides),
  }
}

function updateGatewayScoreRequest(overrides = {}) {
  const latency = overrides.txnLatency || {}

  return {
    merchantId: overrides.merchantId || merchantId('dynamic'),
    gateway: overrides.gateway || gatewayConnector('stripe').gateway_name,
    gatewayReferenceId:
      overrides.gatewayReferenceId === undefined ? null : overrides.gatewayReferenceId,
    status: overrides.status || 'AUTHORIZED',
    paymentId: overrides.paymentId || paymentId('update'),
    enforceDynamicRoutingFailure:
      overrides.enforceDynamicRoutingFailure === undefined
        ? null
        : overrides.enforceDynamicRoutingFailure,
    txnLatency: {
      gatewayLatency: latency.gatewayLatency ?? 3000,
      ...latency,
    },
  }
}

function singleRoutingPayload(createdBy, overrides = {}) {
  return {
    name: overrides.name || ruleName('single'),
    description: overrides.description || 'single connector routing rule',
    created_by: createdBy,
    algorithm_for: overrides.algorithm_for || 'payment',
    algorithm: {
      type: 'single',
      data: gatewayConnector(overrides.gateway || 'stripe', overrides.gateway_id || undefined),
    },
    metadata: overrides.metadata || {},
  }
}

function priorityRoutingPayload(createdBy, overrides = {}) {
  const connectors = overrides.connectors || [
    gatewayConnector('stripe'),
    gatewayConnector('razorpay'),
  ]

  return {
    name: overrides.name || ruleName('priority'),
    description: overrides.description || 'priority routing rule',
    created_by: createdBy,
    algorithm_for: overrides.algorithm_for || 'payment',
    algorithm: {
      type: 'priority',
      data: connectors,
    },
    metadata: overrides.metadata || {},
  }
}

function volumeSplitRoutingPayload(createdBy, overrides = {}) {
  const data = overrides.data || [
    { split: 70, output: gatewayConnector('stripe') },
    { split: 30, output: gatewayConnector('paytm') },
  ]

  return {
    name: overrides.name || ruleName('volume_split'),
    description: overrides.description || 'volume split routing rule',
    created_by: createdBy,
    algorithm_for: overrides.algorithm_for || 'payment',
    algorithm: {
      type: 'volume_split',
      data,
    },
    metadata: overrides.metadata || {},
  }
}

function advancedRoutingPayload(createdBy, overrides = {}) {
  return {
    name: overrides.name || ruleName('advanced'),
    description: overrides.description || 'advanced routing rule',
    created_by: createdBy,
    algorithm_for: overrides.algorithm_for || 'payment',
    algorithm: {
      type: 'advanced',
      data: {
        globals: overrides.globals || {},
        default_selection:
          overrides.default_selection || {
            priority: [gatewayConnector('stripe'), gatewayConnector('adyen')],
          },
        rules:
          overrides.rules || [
            {
              name: 'Card Rule',
              routing_type: 'priority',
              output: {
                priority: [gatewayConnector('checkout'), gatewayConnector('adyen')],
              },
              statements: [
                {
                  condition: [
                    {
                      lhs: 'payment_method',
                      comparison: 'equal',
                      value: { type: 'enum_variant', value: 'card' },
                      metadata: {},
                    },
                    {
                      lhs: 'amount',
                      comparison: 'greater_than',
                      value: { type: 'number', value: 100 },
                      metadata: {},
                    },
                  ],
                },
              ],
            },
          ],
      },
    },
    metadata: overrides.metadata || {},
  }
}

function advancedNestedAndOrRoutingPayload(createdBy, overrides = {}) {
  return {
    name: overrides.name || ruleName('advanced_nested'),
    description: overrides.description || 'advanced nested routing rule',
    created_by: createdBy,
    algorithm_for: overrides.algorithm_for || 'payment',
    algorithm: {
      type: 'advanced',
      data: {
        globals: overrides.globals || {},
        default_selection:
          overrides.default_selection || {
            priority: [gatewayConnector('checkout'), gatewayConnector('adyen')],
          },
        rules:
          overrides.rules || [
            {
              name: 'Nested Network Rule',
              routing_type: 'priority',
              output: {
                priority: [gatewayConnector('stripe'), gatewayConnector('razorpay')],
              },
              statements: [
                {
                  condition: [
                    {
                      lhs: 'payment_method',
                      comparison: 'equal',
                      value: { type: 'enum_variant', value: 'card' },
                      metadata: {},
                    },
                  ],
                  nested: [
                    {
                      condition: [
                        {
                          lhs: 'card_network',
                          comparison: 'equal',
                          value: { type: 'enum_variant', value: 'visa' },
                          metadata: {},
                        },
                      ],
                    },
                    {
                      condition: [
                        {
                          lhs: 'currency',
                          comparison: 'equal',
                          value: { type: 'enum_variant', value: 'USD' },
                          metadata: {},
                        },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
      },
    },
    metadata: overrides.metadata || {},
  }
}

function ruleEvaluatePayload(createdBy, parameters = {}, overrides = {}) {
  return {
    created_by: createdBy,
    parameters: {
      payment_method: { type: 'enum_variant', value: 'upi' },
      amount: { type: 'number', value: 10 },
      ...parameters,
    },
    ...(overrides.fallback_output ? { fallback_output: overrides.fallback_output } : {}),
    ...(overrides.payment_id ? { payment_id: overrides.payment_id } : {}),
  }
}

module.exports = {
  CONNECTORS,
  merchantId,
  paymentId,
  customerId,
  ruleName,
  gatewayConnector,
  connectorNames,
  srConfigData,
  eliminationConfigData,
  debitRoutingConfigData,
  paymentInfo,
  srDecideGatewayRequest,
  updateGatewayScoreRequest,
  singleRoutingPayload,
  priorityRoutingPayload,
  advancedRoutingPayload,
  advancedNestedAndOrRoutingPayload,
  volumeSplitRoutingPayload,
  ruleEvaluatePayload,
}
