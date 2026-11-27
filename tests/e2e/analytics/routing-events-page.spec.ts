import { test, expect } from '../../fixtures/test'
import { clickhouseInsert } from '../../helpers/clickhouse'
import { FLIP_FROM, FLIP_TO, seedLeaderFlip, waitForLeaderFlip } from '../../helpers/routing-events'

/**
 * The Routing events page turns a leader change into something a merchant can act on: who took
 * over from whom, both PSPs' success rates, and a link to the payments behind it.
 */
test.use({ viewport: { width: 1440, height: 1000 } })

test.describe('Routing events UI', () => {
  test('explains a leader change and links to its payments', async ({ api, authedPage, merchant }) => {
    test.setTimeout(150_000)
    await seedLeaderFlip(api, merchant.id)
    await waitForLeaderFlip(api)

    await authedPage.goto('/events')
    await authedPage.getByRole('combobox').selectOption('1h')

    // PSP names are shown the way the rest of the dashboard shows them.
    const [from, to] = [FLIP_FROM, FLIP_TO].map((g) => g.charAt(0).toUpperCase() + g.slice(1))

    // Headline numbers.
    await expect(authedPage.getByText('Leader changes', { exact: true })).toBeVisible({ timeout: 30_000 })
    await expect(authedPage.getByText(`${to} took over Credit card`)).toBeVisible()

    // The change: who took over, by how much, and what moved.
    const row = authedPage
      .locator('li', { has: authedPage.getByRole('link', { name: /View payments/ }) })
      .filter({ hasText: 'Leader change' })
    // The flip is recorded in the second Adyen first led; how far Stripe had fallen by then depends
    // on how many of the seeded failures landed in that same second, so check the shape, not the
    // exact numbers.
    await expect(row).toContainText(/\+\d+\.\d pp/)
    // Each PSP's rate at the change sits next to its name, on one line; the full sentence (with
    // the before→after scores) is the line's tooltip and what screen readers hear.
    await expect(row).toContainText(new RegExp(`${from}\\d+\\.\\d%→${to}100\\.0%`))
    await expect(row).toContainText(new RegExp(`${from} fell from 100\\.0% to \\d+\\.\\d%; ${to} held 100\\.0%\\.`))
    // Events do not name the payment method.
    await expect(row).not.toContainText('Credit card')
    await expect(row).not.toContainText('stale')

    // The drill-down opens the audit filtered to this flip's two PSPs and dimension. Payment
    // audit rows reach ClickHouse on their own path and can trail the score snapshots by a few
    // seconds, so wait until the link's query has rows before following it.
    const href = (await row.getByRole('link', { name: /View payments/ }).getAttribute('href')) ?? ''
    const linkQuery = Object.fromEntries(new URL(href, 'http://dashboard').searchParams)
    await expect
      .poll(
        async () =>
          (await api.raw('GET', '/analytics/payment-audit', { failOnStatusCode: false, qs: { ...linkQuery, page: 1 } }))
            .body?.total_results ?? 0,
        { timeout: 30_000, intervals: [1_000] },
      )
      .toBeGreaterThan(0)
    await row.getByRole('link', { name: /View payments/ }).click()
    await expect(authedPage).toHaveURL(/\/audit\?/)
    const url = new URL(authedPage.url())
    expect(url.searchParams.get('gateway')?.split(',').sort()).toEqual([FLIP_FROM, FLIP_TO].sort())
    expect(url.searchParams.get('payment_method_type')).toBe('CREDIT')
    expect(Number(url.searchParams.get('end_ms'))).toBeLessThanOrEqual(Date.now())
    await expect(authedPage.getByText(/[1-9][\d,]* match(es)? found/)).toBeVisible({ timeout: 30_000 })
  })

  test('gives each dimension set its own column, grouping its back-to-back retunes', async ({ authedPage, merchant, request }) => {
    // Autopilot retunes are emitted by the calibrator, not by a request the test can make, so
    // write them the way it does: domain events tagged autopilot_calibration.
    const minute = 60_000
    const start = Date.now() - 3 * 60 * minute
    const visa = (id: string, atMinute: number, previous: number | null, bucket: number) => ({
      event_id: `retune_${merchant.id}_${id}`,
      api_flow: 'dynamic_routing',
      flow_type: 'autopilot_calibration',
      merchant_id: merchant.id,
      payment_method_type: 'DEBIT',
      payment_method: 'CARD',
      card_network: 'VISA',
      currency: 'USD',
      auth_type: 'THREE_DS',
      event_stage: 'autopilot_calibration',
      status: 'applied',
      score_value: 20,
      sigma_factor: previous == null ? null : 20,
      transaction_count: bucket,
      average_latency: previous,
      created_at_ms: start + atMinute * minute,
    })
    // Six back-to-back Visa retunes, then a Mastercard retune, then two more Visa retunes. The
    // first of those two found its settings cleared (as after a Hard refresh): no previous values.
    const path = [300, 250, 225, 125, 100, 250, 300]
    const rows = [
      ...path.slice(1).map((bucket, i) => visa(String(i), i * 3, path[i], bucket)),
      { ...visa('mastercard', 20, 200, 150), card_network: 'MASTERCARD' },
      visa('restart', 25, null, 275),
      visa('after_restart', 28, 275, 300),
    ]
    await clickhouseInsert(request, 'analytics_domain_events', rows)

    await authedPage.goto('/events')
    await authedPage.getByRole('combobox', { name: 'Time range' }).selectOption('12h')
    // Each dimension set is its own column. The Visa set's eight retunes are back to back there
    // (the Mastercard retune went to the Mastercard column), so they fold into one item.
    const visaColumn = authedPage.getByRole('region', { name: 'Debit card · Visa · USD · 3DS' })
    const mastercardColumn = authedPage.getByRole('region', { name: 'Debit card · Mastercard · USD · 3DS' })
    await expect(visaColumn).toBeVisible({ timeout: 30_000 })
    await expect(mastercardColumn).toBeVisible()
    await expect(mastercardColumn).toContainText('150')
    const run = visaColumn.locator('li', { has: authedPage.getByRole('button', { name: /8 retunes/ }) })
    await expect(run).toContainText('Autopilot · 8 retunes')
    // The path runs through the restart, where the cluster's settings were cleared.
    await expect(run).toContainText('bucket 300 → 250 → 225 → … → 300 → 275 → 300')
    // Hedging never moved, so it is mentioned once as unchanged rather than on every retune.
    await expect(run).toContainText('hedging 20.0% held')
    // The payment method is in the column's name, not on each event.
    await expect(run).not.toContainText('Debit card')

    // Items from the page's first load are not flagged as newly arrived.
    await expect(authedPage.locator('li.bg-sky-50')).toHaveCount(0)

    await run.getByRole('button', { name: /8 retunes/ }).click()
    const steps = run.getByRole('listitem')
    await expect(steps).toHaveCount(8)
    // Newest first.
    await expect(steps.first()).toContainText('bucket size 275 → 300')
    await expect(steps.last()).toContainText('bucket size 300 → 250')
    // The cluster was tuned before, so the retune from scratch started over; it was not its first.
    await expect(steps.nth(1)).toContainText('bucket size 275, hedging 20.0% (started over)')
    await expect(steps.nth(1)).toContainText('restart')
  })
})
