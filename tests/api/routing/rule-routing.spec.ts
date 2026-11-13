import { test, expect, factory } from '../../fixtures/test'
import {
  expectValidRoutingAlgorithmCreateResponse,
  expectValidRoutingAlgorithmList,
} from '../../helpers/assertions'

/**
 * API-contract port of three Cypress specs:
 *   - cypress/e2e/api/rule-routing-single.cy.js
 *   - cypress/e2e/api/rule-routing-priority.cy.js
 *   - cypress/e2e/api/rule-routing-advanced.cy.js
 *
 * The `merchant` fixture replaces the Cypress `waitForService` + `ensureMerchantAccount` beforeEach
 * and auto-cleans the merchant afterwards. The `createdBy` for each routing algorithm is the merchant id,
 * matching the Cypress source. Chained `cy.then` flows become plain `await` sequences.
 */

test.describe('Single Connector Routing API', () => {
  test('creates, activates, lists, and evaluates a single connector algorithm', async ({ api, merchant }) => {
    const m = merchant.id
    const payload = factory.singleRoutingPayload(m, {
      name: factory.ruleName('single_rule'),
      gateway: 'stripe',
    })

    const create = await api.createRoutingAlgorithm(payload)
    expectValidRoutingAlgorithmCreateResponse(create.body)
    const routingAlgorithmId = create.body.rule_id

    const list = await api.listRoutingAlgorithms(m)
    expectValidRoutingAlgorithmList(list.body)
    expect(list.body.some((rule: any) => rule.id === routingAlgorithmId)).toBe(true)

    const activate = await api.activateRoutingAlgorithm(m, routingAlgorithmId)
    expect(activate.status).toBe(200)

    const active = await api.listActiveRoutingAlgorithms(m)
    expectValidRoutingAlgorithmList(active.body)
    expect(active.body.some((rule: any) => rule.id === routingAlgorithmId)).toBe(true)

    const evaluation = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(m))
    expect(['success', 'default_selection']).toContain(evaluation.body.status)
    expect(evaluation.body.output.type).toBe('straight_through')
    expect(evaluation.body.output.connector.gateway_name).toBe('stripe')
  })
})

test.describe('Priority Routing API', () => {
  test('creates, activates, lists, and evaluates a priority algorithm preserving order', async ({ api, merchant }) => {
    const m = merchant.id
    const payload = factory.priorityRoutingPayload(m, {
      name: factory.ruleName('priority_rule'),
      connectors: [
        factory.gatewayConnector('stripe'),
        factory.gatewayConnector('razorpay'),
        factory.gatewayConnector('adyen'),
      ],
    })

    const create = await api.createRoutingAlgorithm(payload)
    expectValidRoutingAlgorithmCreateResponse(create.body)
    const routingAlgorithmId = create.body.rule_id

    const activate = await api.activateRoutingAlgorithm(m, routingAlgorithmId)
    expect(activate.status).toBe(200)

    const list = await api.listRoutingAlgorithms(m)
    expectValidRoutingAlgorithmList(list.body)
    const created = list.body.find((rule: any) => rule.id === routingAlgorithmId)
    expect(created).toBeTruthy()

    const active = await api.listActiveRoutingAlgorithms(m)
    expect(active.body.some((rule: any) => rule.id === routingAlgorithmId)).toBe(true)

    const evaluation = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(m))
    expect(evaluation.body.output.type).toBe('priority')
    const gateways = evaluation.body.output.connectors.map((connector: any) => connector.gateway_name)
    expect(gateways).toEqual(['stripe', 'razorpay', 'adyen'])
  })
})

test.describe('Advanced Routing API', () => {
  test('evaluates default-selection and matched rule paths for a simple advanced algorithm', async ({ api, merchant }) => {
    const m = merchant.id
    const payload = factory.advancedRoutingPayload(m, {
      name: factory.ruleName('advanced_simple'),
    })

    const create = await api.createRoutingAlgorithm(payload)
    const routingAlgorithmId = create.body.rule_id
    await api.activateRoutingAlgorithm(m, routingAlgorithmId)

    const cardEvaluation = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(m, {
        payment_method: { type: 'enum_variant', value: 'card' },
        amount: { type: 'number', value: 150 },
      }),
    )
    expect(['success', 'default_selection']).toContain(cardEvaluation.body.status)
    expect(cardEvaluation.body.output.type).toBe('priority')
    expect(cardEvaluation.body.output.connectors[0].gateway_name).toBe('checkout')

    const upiEvaluation = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(m, {
        payment_method: { type: 'enum_variant', value: 'upi' },
        amount: { type: 'number', value: 50 },
      }),
    )
    expect(['success', 'default_selection']).toContain(upiEvaluation.body.status)
    expect(upiEvaluation.body.output.type).toBe('priority')
    expect(upiEvaluation.body.output.connectors[0].gateway_name).toBe('stripe')
  })

  test('supports nested AND/OR style routing evaluation via nested statements', async ({ api, merchant }) => {
    const m = merchant.id
    const payload = factory.advancedNestedAndOrRoutingPayload(m, {
      name: factory.ruleName('advanced_nested'),
    })

    const create = await api.createRoutingAlgorithm(payload)
    const routingAlgorithmId = create.body.rule_id
    await api.activateRoutingAlgorithm(m, routingAlgorithmId)

    const cardVisaEvaluation = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(m, {
        payment_method: { type: 'enum_variant', value: 'card' },
        card_network: { type: 'enum_variant', value: 'visa' },
      }),
    )
    expect(['success', 'default_selection']).toContain(cardVisaEvaluation.body.status)
    expect(cardVisaEvaluation.body.output.connectors[0].gateway_name).toBe('stripe')

    const cardUsdEvaluation = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(m, {
        payment_method: { type: 'enum_variant', value: 'card' },
        currency: { type: 'enum_variant', value: 'USD' },
      }),
    )
    expect(['success', 'default_selection']).toContain(cardUsdEvaluation.body.status)
    expect(cardUsdEvaluation.body.output.connectors[0].gateway_name).toBe('stripe')

    const upiUsdEvaluation = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(m, {
        payment_method: { type: 'enum_variant', value: 'upi' },
        currency: { type: 'enum_variant', value: 'USD' },
      }),
    )
    expect(['success', 'default_selection']).toContain(upiUsdEvaluation.body.status)
    expect(upiUsdEvaluation.body.output.connectors[0].gateway_name).toBe('checkout')
  })
})
