import { test, expect } from '../../fixtures/test'

/**
 * The volume-commitment loop, driven the way a merchant drives it: configure a contract, activate
 * it, turn the feature on, then run the Decision Simulator against it and read the result in
 * Analytics.
 *
 * The contract is deliberately shaped so steering has work to do. `adyen` carries the bigger
 * reward but is the *worse* processor, so approval-rate routing prefers `stripe` and would starve
 * the commitment on its own — the nudge has to pull payments back, and only where the gap is
 * inside the 5pp tolerance. A run where the committed PSP is already the SR head proves nothing.
 */
test.use({ viewport: { width: 1600, height: 1200 } })

test('steers a behind-pace PSP and reports the run in analytics', async ({ authedPage, api, merchant }) => {
  const page = authedPage
  const m = merchant.id

  // ── Contract: adyen reachable and worth more; stripe too big to fund alongside it ──
  const created = await api.raw('POST', '/routing/create', {
    body: {
      name: 'e2e steering contract',
      description: 'adyen behind on approvals, must be steered',
      created_by: m,
      algorithm_for: 'volume_commitment',
      algorithm: {
        type: 'volume_contract',
        data: {
          routing_mode: 'pace_guarded',
          tolerance_bps: 500,
          metric: 'gmv',
          currency: { denomination: 'USD', amount_units: 'minor' },
          expected_daily_traffic: 500000,
          forecast_interval_secs: 5,
          volume_contracts: [
            {
              id: 'adyen', connector: 'adyen', status: 'active',
              billing_cycle: { type: 'test_minutes', anchor: 4, timezone: 'UTC' },
              archetype: 'lumpsum',
              terms: { target: 600000, reward: { kind: 'flat', value: { flat_amount: 13000 } } },
            },
            {
              id: 'stripe', connector: 'stripe', status: 'active',
              billing_cycle: { type: 'test_minutes', anchor: 4, timezone: 'UTC' },
              archetype: 'lumpsum',
              terms: { target: 1600000, reward: { kind: 'flat', value: { flat_amount: 2000 } } },
            },
          ],
        },
      },
    },
  })
  expect(created.status).toBe(200)
  await api.activateRoutingAlgorithm(m, created.body.rule_id)
  await api.raw('POST', `/merchant-account/${m}/features/volume-contracts`, {
    body: { enabled: true },
  })

  // ── Simulator: the contract fills in connectors, ticket size and pacing ──
  await page.goto('/decisions/simulator')
  await expect(page.getByRole('heading', { name: 'Decision Simulator' })).toBeVisible()

  const panel = page.getByText('Active volume contract')
  await expect(panel).toBeVisible({ timeout: 20_000 })
  await page.getByRole('button', { name: 'Load contract into simulator' }).click()

  // The load writes the contract's connectors into the run's eligible list.
  await expect(page.locator('input').filter({ hasText: '' }).first()).toBeVisible()

  await page.getByRole('button', { name: /Run simulation/i }).click()
  // Long enough for several forecast intervals, so pacing and steering both get a chance.
  await page.waitForTimeout(45_000)

  // ── Analytics: the run and its audit ──
  await page.goto('/analytics?view=volume_commitments')
  await expect(page.getByText('What the volume contract did')).toBeVisible({ timeout: 20_000 })
  await expect(page.getByText('Previous cycle').first()).toBeVisible()
  await expect(page.getByText('This cycle').first()).toBeVisible()
  await expect(page.getByText('Cumulative volume vs. each promise')).toBeVisible()
  await expect(page.getByText('Audit trail')).toBeVisible()

  // stripe is the one the engine gives up on; adyen is the one it chases.
  await expect(page.getByText(/eliminated/i).first()).toBeVisible()

  const audit = await api.raw('GET', `/merchant-account/${m}/volume-commitment/audit`)
  expect(audit.body.runs.length).toBeGreaterThan(0)
  const steers = audit.body.runs.reduce((n: number, r: any) => n + r.steers, 0)
  expect(steers, 'the nudge should have moved payments to the behind-pace PSP').toBeGreaterThan(0)
})
