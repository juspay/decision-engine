// Test Data Factory for Decision Engine Tests
// This file provides utilities to generate test data for different routing scenarios

const { v4: uuidv4 } = require('uuid')

class TestDataFactory {
  static generateMerchantId(prefix = 'merc_test_') {
    return `${prefix}${Date.now()}_${Math.floor(Math.random() * 1000)}`
  }

  static generatePaymentId(prefix = 'PAY_test_') {
    return `${prefix}${Date.now()}_${Math.floor(Math.random() * 1000)}`
  }

  static generateCustomerId(prefix = 'CUST_test_') {
    return `${prefix}${Date.now()}_${Math.floor(Math.random() * 1000)}`
  }

  // Success Rate Rule Configurations
  static getSuccessRateRuleConfig(options = {}) {
    return {
      type: "successRate",
      data: {
        defaultLatencyThreshold: options.latencyThreshold || 90,
        defaultSuccessRate: options.successRate || 0.5,
        defaultBucketSize: options.bucketSize || 200,
        defaultHedgingPercent: options.hedgingPercent || 5,
        txnLatency: {
          gatewayLatency: options.gatewayLatency || 5000
        },
        subLevelInputConfig: options.subLevelConfig || [
          {
            paymentMethodType: "upi",
            paymentMethod: "upi_collect",
            bucketSize: 250,
            hedgingPercent: 1
          }
        ]
      }
    }
  }

  // Payment Latency Rule Configurations
  static getPaymentLatencyRuleConfig(options = {}) {
    return {
      type: "paymentLatency",
      data: {
        defaultLatencyThreshold: options.latencyThreshold || 90,
        defaultBucketSize: options.bucketSize || 200,
        defaultHedgingPercent: options.hedgingPercent || 5,
        txnLatency: {
          gatewayLatency: options.gatewayLatency || 3000,
          paymentLatency: options.paymentLatency || 5000
        },
        subLevelInputConfig: options.subLevelConfig || []
      }
    }
  }

  // Cost Based Rule Configurations (if supported)
  static getCostBasedRuleConfig(options = {}) {
    return {
      type: "costBased",
      data: {
        defaultCostThreshold: options.costThreshold || 2.5,
        defaultBucketSize: options.bucketSize || 200,
        defaultHedgingPercent: options.hedgingPercent || 5,
        costConfig: {
          baseCost: options.baseCost || 1.0,
          variableCost: options.variableCost || 0.5
        },
        subLevelInputConfig: options.subLevelConfig || []
      }
    }
  }

  // Payment Info Configurations
  static getPaymentInfo(options = {}) {
    return {
      paymentId: options.paymentId || this.generatePaymentId(),
      amount: options.amount || 100.50,
      currency: options.currency || "USD",
      customerId: options.customerId || this.generateCustomerId(),
      udfs: options.udfs || null,
      preferredGateway: options.preferredGateway || null,
      paymentType: options.paymentType || "ORDER_PAYMENT",
      metadata: options.metadata || null,
      internalMetadata: options.internalMetadata || null,
      isEmi: options.isEmi || false,
      emiBank: options.emiBank || null,
      emiTenure: options.emiTenure || null,
      paymentMethodType: options.paymentMethodType || "UPI",
      paymentMethod: options.paymentMethod || "UPI_PAY",
      paymentSource: options.paymentSource || null,
      authType: options.authType || null,
      cardIssuerBankName: options.cardIssuerBankName || null,
      cardIsin: options.cardIsin || null,
      cardType: options.cardType || null,
      cardSwitchProvider: options.cardSwitchProvider || null
    }
  }

  // Gateway Decision Request
  static getGatewayDecisionRequest(options = {}) {
    return {
      merchantId: options.merchantId || this.generateMerchantId(),
      eligibleGatewayList: options.eligibleGatewayList || ["GatewayA", "GatewayB", "GatewayC"],
      rankingAlgorithm: options.rankingAlgorithm || "SR_BASED_ROUTING",
      eliminationEnabled: options.eliminationEnabled !== undefined ? options.eliminationEnabled : true,
      paymentInfo: this.getPaymentInfo(options.paymentInfo || {})
    }
  }

  // Score Update Request
  static getScoreUpdateRequest(options = {}) {
    return {
      merchantId: options.merchantId || this.generateMerchantId(),
      gateway: options.gateway || "GatewayA",
      gatewayReferenceId: options.gatewayReferenceId || null,
      status: options.status || "AUTHORIZED",
      paymentId: options.paymentId || this.generatePaymentId(),
      enforceDynamicRoutingFailure: options.enforceDynamicRoutingFailure || null,
      txnLatency: {
        gatewayLatency: options.gatewayLatency || 3000,
        paymentLatency: options.paymentLatency || 4000,
        ...options.txnLatency
      }
    }
  }

  // Test Scenarios
  static getLatencyTestScenarios() {
    return [
      { name: "Low Latency", gatewayLatency: 1000, paymentLatency: 1500, status: "AUTHORIZED" },
      { name: "Medium Latency", gatewayLatency: 3000, paymentLatency: 4000, status: "AUTHORIZED" },
      { name: "High Latency", gatewayLatency: 6000, paymentLatency: 8000, status: "AUTHORIZED" },
      { name: "Failed Transaction", gatewayLatency: 2000, paymentLatency: 3000, status: "FAILED" },
      { name: "Timeout", gatewayLatency: 10000, paymentLatency: 12000, status: "TIMEOUT" }
    ]
  }

  static getSuccessRateTestScenarios() {
    return [
      { name: "High Success", transactions: Array(8).fill("AUTHORIZED").concat(Array(2).fill("FAILED")) },
      { name: "Medium Success", transactions: Array(6).fill("AUTHORIZED").concat(Array(4).fill("FAILED")) },
      { name: "Low Success", transactions: Array(3).fill("AUTHORIZED").concat(Array(7).fill("FAILED")) },
      { name: "All Success", transactions: Array(10).fill("AUTHORIZED") },
      { name: "All Failed", transactions: Array(10).fill("FAILED") }
    ]
  }

  static getPaymentMethodTestCases() {
    return [
      { type: "UPI", method: "UPI_PAY", description: "UPI Payment" },
      { type: "upi", method: "upi_collect", description: "UPI Collect" },
      { type: "CARD", method: "CARD_PAY", description: "Card Payment" },
      { type: "NETBANKING", method: "NETBANKING_PAY", description: "Net Banking" },
      { type: "WALLET", method: "WALLET_PAY", description: "Wallet Payment" }
    ]
  }

  static getRoutingAlgorithmTestCases() {
    return [
      { algorithm: "SR_BASED_ROUTING", description: "Success Rate Based Routing" },
      { algorithm: "PL_BASED_ROUTING", description: "Payment Latency Based Routing" },
      { algorithm: "COST_BASED_ROUTING", description: "Cost Based Routing" }
    ]
  }

  // Edge Case Scenarios
  static getEdgeCaseScenarios() {
    return {
      extremeLatency: {
        gatewayLatency: 30000,
        paymentLatency: 45000,
        status: "TIMEOUT"
      },
      zeroLatency: {
        gatewayLatency: 0,
        paymentLatency: 0,
        status: "AUTHORIZED"
      },
      negativeAmount: {
        amount: -100,
        currency: "USD"
      },
      largeAmount: {
        amount: 999999.99,
        currency: "USD"
      },
      invalidCurrency: {
        amount: 100,
        currency: "INVALID"
      }
    }
  }

  // Load Testing Data
  static generateLoadTestData(count = 100) {
    return Array.from({ length: count }, (_, index) => ({
      merchantId: this.generateMerchantId(`load_test_${index}_`),
      paymentId: this.generatePaymentId(`load_pay_${index}_`),
      customerId: this.generateCustomerId(`load_cust_${index}_`),
      amount: Math.floor(Math.random() * 1000) + 1,
      latency: Math.floor(Math.random() * 5000) + 500
    }))
  }

  // Performance Test Configurations
  static getPerformanceTestConfig() {
    return {
      concurrentUsers: [1, 5, 10, 20, 50],
      requestsPerSecond: [1, 5, 10, 25, 50],
      testDuration: [30, 60, 120, 300], // seconds
      latencyThresholds: [1000, 2000, 3000, 5000] // milliseconds
    }
  }
}

module.exports = TestDataFactory
