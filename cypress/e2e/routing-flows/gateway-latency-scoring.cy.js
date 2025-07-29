describe('Gateway Latency Scoring Flow', () => {
  let testData = {}

  beforeEach(() => {
    // Wait for service to be ready
    cy.waitForService()
    
    // Generate unique test data for each test
    testData = {
      merchantId: `merc_${Date.now()}`,
      paymentId: `PAY_${Date.now()}`,
      customerId: `CUST${Date.now()}`
    }
  })

  afterEach(() => {
    // Clean up test data if needed
    cy.cleanupTestData(testData.merchantId)
  })

  it('should create merchant account successfully', { tags: ['@merchant'] }, () => {
    cy.createMerchantAccount(testData.merchantId).then((result) => {
      expect(result.merchantId).to.equal(testData.merchantId)
      expect(result.response).to.exist
    })
  })

  it('should create success rate routing rule successfully', { tags: ['@routing-rule'] }, () => {
    cy.createMerchantAccount(testData.merchantId).then(() => {
      return cy.createSuccessRateRule(testData.merchantId, {
        latencyThreshold: 90,
        successRate: 0.5,
        bucketSize: 200,
        hedgingPercent: 5,
        gatewayLatency: 5000
      })
    }).then((result) => {
      expect(result.ruleConfig).to.have.property('type', 'successRate')
      expect(result.ruleConfig.data).to.have.property('defaultLatencyThreshold', 90)
      expect(result.ruleConfig.data).to.have.property('defaultSuccessRate', 0.5)
      expect(result.response).to.exist
    })
  })

  it('should decide gateway successfully', { tags: ['@gateway-decision'] }, () => {
    cy.createMerchantAccount(testData.merchantId).then(() => {
      return cy.createSuccessRateRule(testData.merchantId)
    }).then(() => {
      return cy.decideGateway({
        merchantId: testData.merchantId,
        eligibleGatewayList: ["GatewayA", "GatewayB", "GatewayC"],
        rankingAlgorithm: "SR_BASED_ROUTING",
        eliminationEnabled: true,
        paymentInfo: {
          paymentId: testData.paymentId,
          amount: 100.50,
          currency: "USD",
          customerId: testData.customerId,
          paymentMethodType: "UPI",
          paymentMethod: "UPI_PAY"
        }
      })
    }).then((result) => {
      expect(result.response).to.haveValidGatewayResponse()
      expect(result.response.decided_gateway).to.be.oneOf(["GatewayA", "GatewayB", "GatewayC"])
    })
  })

  it('should update gateway score successfully', { tags: ['@score-update'] }, () => {
    let selectedGateway

    cy.createMerchantAccount(testData.merchantId).then(() => {
      return cy.createSuccessRateRule(testData.merchantId)
    }).then(() => {
      return cy.decideGateway({
        merchantId: testData.merchantId,
        paymentInfo: {
          paymentId: testData.paymentId
        }
      })
    }).then((result) => {
      selectedGateway = result.response.decided_gateway
      return cy.updateGatewayScore({
        merchantId: testData.merchantId,
        gateway: selectedGateway,
        paymentId: testData.paymentId,
        status: "AUTHORIZED",
        txnLatency: {
          gatewayLatency: 6000
        }
      })
    }).then((result) => {
      expect(result.response).to.haveValidScoreUpdate()
    })
  })

  it('should show different gateway selection after score update', { tags: ['@score-impact'] }, () => {
    let initialGateway, updatedGateway, initialGatewayScore, initialGatewayCurrentScore

    cy.createMerchantAccount(testData.merchantId).then(() => {
      return cy.createSuccessRateRule(testData.merchantId, {
        gatewayLatency: 3000 // Lower threshold to make latency impact more visible
      })
    }).then(() => {
      // First gateway decision
      return cy.decideGateway({
        merchantId: testData.merchantId,
        paymentInfo: {
          paymentId: testData.paymentId
        }
      })
    }).then((result) => {
      initialGateway = result.response.decided_gateway
      initialGatewayScore = result.response.gateway_priority_map[initialGateway]
      
      // Update score with high latency
      return cy.updateGatewayScore({
        merchantId: testData.merchantId,
        gateway: initialGateway,
        paymentId: testData.paymentId,
        status: "AUTHORIZED",
        txnLatency: {
          gatewayLatency: 8000 // High latency to impact scoring
        }
      })
    }).then(() => {
      // Second gateway decision to see if scoring changed
      return cy.decideGateway({
        merchantId: testData.merchantId,
        paymentInfo: {
          paymentId: `PAY_${Date.now()}_2` // Different payment ID
        }
      })
    }).then((result) => {
      initialGatewayCurrentScore = result.response.gateway_priority_map[initialGateway]
      updatedGateway = result.response.decided_gateway
      
      // Log both gateways for comparison
      cy.log(`Initial Gateway: ${initialGateway}`)
      cy.log(`Updated Gateway: ${updatedGateway}`)
      
      expect(updatedGateway).to.not.equal(initialGateway);
      expect(initialGatewayCurrentScore).to.be.lessThan(initialGatewayScore);
    })
  })

  it('should handle different payment methods', { tags: ['@payment-methods'] }, () => {
    const paymentMethods = [
      { type: "UPI", method: "UPI_PAY" },
      { type: "upi", method: "upi_collect" },
      { type: "CARD", method: "CARD_PAY" }
    ]

    cy.createMerchantAccount(testData.merchantId).then(() => {
      return cy.createSuccessRateRule(testData.merchantId)
    }).then(() => {
      // Test each payment method
      paymentMethods.forEach((paymentMethod, index) => {
        cy.decideGateway({
          merchantId: testData.merchantId,
          paymentInfo: {
            paymentId: `${testData.paymentId}_${index}`,
            paymentMethodType: paymentMethod.type,
            paymentMethod: paymentMethod.method
          }
        }).then((result) => {
          expect(result.response).to.haveValidGatewayResponse()
          cy.log(`Payment Method ${paymentMethod.type}/${paymentMethod.method}: Gateway ${result.response.decided_gateway}`)
        })
      })
    })
  })

  it('should handle different routing algorithms', { tags: ['@routing-algorithms'] }, () => {
    const algorithms = [
      "SR_BASED_ROUTING",
      "PL_BASED_ROUTING"
    ]

    cy.createMerchantAccount(testData.merchantId).then(() => {
      return cy.createSuccessRateRule(testData.merchantId)
    }).then(() => {
      // Test each routing algorithm
      algorithms.forEach((algorithm, index) => {
        cy.decideGateway({
          merchantId: testData.merchantId,
          rankingAlgorithm: algorithm,
          paymentInfo: {
            paymentId: `${testData.paymentId}_${algorithm}_${index}`
          }
        }).then((result) => {
          expect(result.response).to.haveValidGatewayResponse()
          cy.log(`Algorithm ${algorithm}: Gateway ${result.response.decided_gateway}`)
        })
      })
    })
  })
})
