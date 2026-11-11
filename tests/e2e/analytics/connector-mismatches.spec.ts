import { test, expect, factory, poll } from '../../fixtures/test'

test('Decision Audit filters mismatches and opens their existing full timeline', async ({ api, merchant, authedPage }, testInfo) => {
  test.setTimeout(120_000)
  await authedPage.setViewportSize({ width: 1600, height: 1100 })
  await api.createSuccessRateConfig(merchant.id)
  const paymentId = factory.paymentId('audit_mismatch')
  const matchingId = factory.paymentId('audit_matching')
  for (const id of [paymentId, matchingId]) {
    const decision = await api.decideGateway(factory.srDecideGatewayRequest({
      merchantId: merchant.id, eligibleGatewayList: ['adyen'], paymentInfo: { paymentId: id },
    }))
    await api.updateGatewayScore(factory.updateGatewayScoreRequest({
      merchantId: merchant.id, paymentId: id, gateway: decision.body.decided_gateway,
    }))
  }
  for (const gateway of ['stripe', 'cybersource']) {
    await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId: merchant.id, paymentId, gateway }))
  }
  await poll(
    () => api.raw('GET', '/analytics/payment-audit', { qs: { range: '1d', connector_mismatch: true } }),
    ({ body }) => body.total_results === 1 && body.results[0]?.event_count === 4,
    { message: 'Mismatch should be searchable with its complete event history', timeout: 60_000 },
  )
  await authedPage.goto('/audit')
  await authedPage.getByRole('button', { name: 'More filters' }).click()
  const toggle = authedPage.getByRole('checkbox', { name: 'Connector mismatches only' })
  const matchingRow = authedPage.getByRole('button').filter({ hasText: matchingId })
  await expect(matchingRow).toBeVisible()
  await toggle.check()
  await expect(authedPage).toHaveURL(/connector_mismatch=true/)
  await expect(matchingRow).toHaveCount(0)
  const mismatchRow = authedPage.getByRole('button').filter({ hasText: paymentId })
  await expect(mismatchRow).toContainText('adyen')
  await expect(mismatchRow).toContainText('stripe')
  await expect(mismatchRow).toContainText('cybersource')
  await authedPage.screenshot({ path: testInfo.outputPath('audit-mismatch-filter.png') })

  await authedPage.reload()
  await authedPage.getByRole('button', { name: 'More filters' }).click()
  await expect(toggle).toBeChecked()
  await mismatchRow.click()
  await expect(authedPage.getByRole('button', { name: 'Back to results' })).toBeVisible()
  await expect(authedPage.getByText('4 events', { exact: true })).toBeVisible()
  await authedPage.getByRole('button', { name: 'Back to results' }).click()
  await toggle.uncheck()
  await expect(authedPage).not.toHaveURL(/connector_mismatch=true/)
  await expect(matchingRow).toBeVisible()
  await toggle.check()
  await authedPage.getByRole('button', { name: 'Clear', exact: true }).click()
  await expect(toggle).not.toBeChecked()
  await expect(matchingRow).toBeVisible()
})
