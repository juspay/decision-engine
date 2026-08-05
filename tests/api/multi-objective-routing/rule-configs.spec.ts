import { test, expect } from '../../fixtures/test'
import { expectValidRuleConfigResponse } from '../../helpers/assertions'

/**
 * API-contract port of cypress/e2e/api/rule-configs.cy.js.
 *
 * The `merchant` fixture replaces the Cypress `waitForService` + `ensureMerchantAccount` beforeEach
 * (fresh merchant + dashboard session so /rule/* protected routes authenticate) and auto-cleans the
 * merchant afterwards, standing in for the `cleanupTestData` afterEach.
 */
test.describe('Rule Config CRUD API', () => {
  test('creates, fetches, updates, and deletes success-rate config', async ({ api, merchant }) => {
    const m = merchant.id

    const created = await api.createSuccessRateConfig(m, {
      defaultSuccessRate: 0.5,
      defaultBucketSize: 200,
    })
    expectValidRuleConfigResponse(created.body, 'successRate')

    let got = await api.getSuccessRateConfig(m)
    expectValidRuleConfigResponse(got.body, 'successRate')
    expect(got.body.config.data.defaultBucketSize).toBe(200)

    const updated = await api.updateSuccessRateConfig(m, {
      defaultSuccessRate: 0.7,
      defaultBucketSize: 240,
    })
    expectValidRuleConfigResponse(updated.body, 'successRate')

    got = await api.getSuccessRateConfig(m)
    expect(got.body.config.data.defaultBucketSize).toBe(240)
    // defaultHedgingPercent was not overridden, so it stays at the factory default (5).
    expect(got.body.config.data.defaultHedgingPercent).toBe(5)

    const del = await api.deleteSuccessRateConfig(m)
    expect(del.status).toBe(200)

    const afterDelete = await api.getSuccessRateConfig(m, { failOnStatusCode: false })
    expect(afterDelete.status).not.toBe(200)
  })

  test('creates, fetches, updates, and deletes elimination config', async ({ api, merchant }) => {
    const m = merchant.id

    const created = await api.createEliminationConfig(m, {
      threshold: 0.35,
      txnLatency: { gatewayLatency: 4500 },
    })
    expectValidRuleConfigResponse(created.body, 'elimination')

    let got = await api.getEliminationConfig(m)
    expectValidRuleConfigResponse(got.body, 'elimination')
    expect(got.body.config.data.threshold).toBe(0.35)

    const updated = await api.updateEliminationConfig(m, {
      threshold: 0.55,
      txnLatency: { gatewayLatency: 6500 },
    })
    expectValidRuleConfigResponse(updated.body, 'elimination')

    got = await api.getEliminationConfig(m)
    expect(got.body.config.data.threshold).toBe(0.55)
    expect(got.body.config.data.txnLatency.gatewayLatency).toBe(6500)

    const del = await api.deleteEliminationConfig(m)
    expect(del.status).toBe(200)

    const afterDelete = await api.getEliminationConfig(m, { failOnStatusCode: false })
    expect(afterDelete.status).not.toBe(200)
  })
})
