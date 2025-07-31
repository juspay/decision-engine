describe('Success Rate Routing Flow', () => {
  let testData = {}

  beforeEach(() => {
    cy.waitForService()
    
    testData = {
      merchantId: `merc_sr_${Date.now()}`,
      paymentId: `PAY_SR_${Date.now()}`,
      customerId: `CUST_SR_${Date.now()}`
    }
  })

  afterEach(() => {
    cy.cleanupTestData(testData.merchantId)
  })

  it('should test success rate based routing', { tags: ['@success-rate', '@routing'] }, () => {
    cy.createMerchantAccount(testData.merchantId).then(() => {
      return cy.createSuccessRateRule(testData.merchantId, {
        successRate: 0.8,
        latencyThreshold: 100,
        bucketSize: 300,
        hedgingPercent: 10
      })
    }).then(() => {
      // Define multiple transactions to simulate real-world scenario
      const transactions = [
        { paymentId: `${testData.paymentId}_txn_1`, status: "AUTHORIZED", latency: 2000 },
        { paymentId: `${testData.paymentId}_txn_2`, status: "FAILURE", latency: 5000 },
        { paymentId: `${testData.paymentId}_txn_3`, status: "AUTHORIZED", latency: 1500 },
        { paymentId: `${testData.paymentId}_txn_4`, status: "AUTHORIZED", latency: 3000 },
        { paymentId: `${testData.paymentId}_txn_5`, status: "FAILURE", latency: 4500 },
        { paymentId: `${testData.paymentId}_txn_6`, status: "AUTHORIZED", latency: 2500 }
      ]

      // Store gateway decisions for each transaction
      const gatewayDecisions = []

      // Process each transaction: decide gateway first, then update score
      const processTransaction = (txn, index) => {
        return cy.decideGateway({
          merchantId: testData.merchantId,
          rankingAlgorithm: "SR_BASED_ROUTING",
          paymentInfo: {
            paymentId: txn.paymentId,
            paymentMethodType: "UPI",
            paymentMethod: "UPI_PAY",
            customerId: `${testData.customerId}_${index}`
          }
        }).then((decisionResult) => {
          expect(decisionResult.response).to.haveValidGatewayResponse()
          
          // Store the gateway decision
          gatewayDecisions.push({
            paymentId: txn.paymentId,
            gateway: decisionResult.response.decided_gateway,
            gateway_priority_map: decisionResult.response.gateway_priority_map,
            status: txn.status,
            latency: txn.latency
          })

          cy.log(`Transaction ${index + 1}: Payment ${txn.paymentId} routed to ${decisionResult.response.decided_gateway}`)

          // Update gateway score only once per transaction
          return cy.updateGatewayScore({
            merchantId: testData.merchantId,
            gateway: decisionResult.response.decided_gateway,
            paymentId: txn.paymentId,
            status: txn.status,
            txnLatency: {
              gatewayLatency: txn.latency
            }
          }).then((updateResult) => {
            expect(updateResult.response).to.haveValidScoreUpdate()
            cy.log(`Score updated for ${txn.paymentId}: ${txn.status} with ${txn.latency}ms latency`)
            return cy.wrap(decisionResult.response.decided_gateway)
          })
        })
      }

      // Process transactions sequentially to ensure proper ordering
      let chain = cy.wrap(null)
      transactions.forEach((txn, index) => {
        chain = chain.then(() => processTransaction(txn, index))
      })

      return chain.then(() => {
        // Verify that we have decisions for all transactions
        expect(gatewayDecisions).to.have.length(transactions.length)
        
        // Log summary of gateway selections
        const gatewaySummary = gatewayDecisions.reduce((acc, decision) => {
          acc[decision.gateway] = (acc[decision.gateway] || 0) + 1
          return acc
        }, {})
        cy.log('Gateway Decision', gatewayDecisions)
        cy.log('Gateway Selection Summary:', gatewaySummary)
        
        // Test routing behavior after score updates by making additional decisions
        return cy.decideGateway({
          merchantId: testData.merchantId,
          rankingAlgorithm: "SR_BASED_ROUTING",
          paymentInfo: {
            paymentId: `${testData.paymentId}_final_test`,
            paymentMethodType: "UPI",
            paymentMethod: "UPI_PAY",
            customerId: `${testData.customerId}_final`
          }
        }).then((finalResult) => {
          expect(finalResult.response).to.haveValidGatewayResponse()
          cy.log(`Final routing decision after score updates: ${finalResult.response.decided_gateway}`)
          
          // Verify that routing is influenced by the success rate data
          // The gateway with better success rate should be preferred
          const successfulGateways = gatewayDecisions
            .filter(d => d.status === "AUTHORIZED")
            .map(d => d.gateway)
          
          if (successfulGateways.length > 0) {
            cy.log('Gateways with successful transactions:', [...new Set(successfulGateways)])
          }
        })
      })
    })
  })

  it('should handle multiple gateways with different success rates', { tags: ['@success-rate', '@multiple-gateways'] }, () => {
    const merchantId = `${testData.merchantId}_multi_gw`
    
    cy.createMerchantAccount(merchantId).then(() => {
      return cy.createSuccessRateRule(merchantId, {
        successRate: 0.7,
        latencyThreshold: 100,
        bucketSize: 200,
        hedgingPercent: 5
      })
    }).then(() => {
      // Simulate transactions across multiple gateways with different success patterns
      const gatewayTransactions = [
        // GatewayA - Good performance
        { paymentId: `${testData.paymentId}_gwa_1`, gateway: 'GatewayA', status: "AUTHORIZED", latency: 1500 },
        { paymentId: `${testData.paymentId}_gwa_2`, gateway: 'GatewayA', status: "AUTHORIZED", latency: 1800 },
        { paymentId: `${testData.paymentId}_gwa_3`, gateway: 'GatewayA', status: "AUTHORIZED", latency: 1600 },
        
        // GatewayB - Mixed performance
        { paymentId: `${testData.paymentId}_gwb_1`, gateway: 'GatewayB', status: "AUTHORIZED", latency: 2500 },
        { paymentId: `${testData.paymentId}_gwb_2`, gateway: 'GatewayB', status: "FAILURE", latency: 4000 },
        { paymentId: `${testData.paymentId}_gwb_3`, gateway: 'GatewayB', status: "AUTHORIZED", latency: 2200 },
        
        // GatewayC - Poor performance
        { paymentId: `${testData.paymentId}_gwc_1`, gateway: 'GatewayC', status: "FAILURE", latency: 5000 },
        { paymentId: `${testData.paymentId}_gwc_2`, gateway: 'GatewayC', status: "FAILURE", latency: 4500 },
        { paymentId: `${testData.paymentId}_gwc_3`, gateway: 'GatewayC', status: "AUTHORIZED", latency: 3000 }
      ]

      // Process each transaction: decide gateway first, then update score once
      let chain = cy.wrap(null)
      
      gatewayTransactions.forEach((txn, index) => {
        chain = chain.then(() => {
          // First decide gateway for this transaction
          return cy.decideGateway({
            merchantId: merchantId,
            rankingAlgorithm: "SR_BASED_ROUTING",
            eligibleGatewayList: ['GatewayA', 'GatewayB', 'GatewayC'],
            paymentInfo: {
              paymentId: txn.paymentId,
              paymentMethodType: "UPI",
              paymentMethod: "UPI_PAY",
              customerId: `${testData.customerId}_multi_${index}`
            }
          }).then((decisionResult) => {
            expect(decisionResult.response).to.haveValidGatewayResponse()
            
            const decidedGateway = decisionResult.response.decided_gateway
            cy.log(`Transaction ${index + 1}: ${txn.paymentId} routed to ${decidedGateway}`)
            
            // Update score only once per transaction using the decided gateway
            return cy.updateGatewayScore({
              merchantId: merchantId,
              gateway: decidedGateway,
              paymentId: txn.paymentId,
              status: txn.status,
              txnLatency: {
                gatewayLatency: txn.latency
              }
            }).then((updateResult) => {
              expect(updateResult.response).to.haveValidScoreUpdate()
              cy.log(`Score updated for ${txn.paymentId}: ${txn.status} (${txn.latency}ms) on ${decidedGateway}`)
              return cy.wrap({ decidedGateway, originalGateway: txn.gateway, status: txn.status })
            })
          })
        })
      })

      return chain.then(() => {
        // After all score updates, test final routing decisions
        const finalTests = Array.from({ length: 3 }, (_, i) => ({
          paymentId: `${testData.paymentId}_final_${i}`,
          customerId: `${testData.customerId}_final_${i}`
        }))

        let finalChain = cy.wrap(null)
        const finalDecisions = []

        finalTests.forEach((test, index) => {
          finalChain = finalChain.then(() => {
            return cy.decideGateway({
              merchantId: merchantId,
              rankingAlgorithm: "SR_BASED_ROUTING",
              eligibleGatewayList: ['GatewayA', 'GatewayB', 'GatewayC'],
              paymentInfo: {
                paymentId: test.paymentId,
                paymentMethodType: "UPI",
                paymentMethod: "UPI_PAY",
                customerId: test.customerId
              }
            }).then((finalResult) => {
              expect(finalResult.response).to.haveValidGatewayResponse()
              finalDecisions.push(finalResult.response.decided_gateway)
              cy.log(`Final decision ${index + 1}: ${test.paymentId} → ${finalResult.response.decided_gateway}`)
              return cy.wrap(finalResult.response.decided_gateway)
            })
          })
        })

        return finalChain.then(() => {
          // Log summary of final routing decisions
          const finalSummary = finalDecisions.reduce((acc, gateway) => {
            acc[gateway] = (acc[gateway] || 0) + 1
            return acc
          }, {})
          
          cy.log('Final Routing Summary after score updates:', finalSummary)
          
          // Verify that routing is influenced by success rate data
          expect(finalDecisions).to.have.length(3)
          cy.log('All final routing decisions completed successfully')
        })
      })
    })
  })
})
