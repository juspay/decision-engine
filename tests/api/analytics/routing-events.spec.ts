import { test, expect } from '../../fixtures/test'
import { FLIP_DIMENSION, FLIP_FROM, FLIP_TO, seedLeaderFlip, waitForLeaderFlip } from '../../helpers/routing-events'

/**
 * `/analytics/routing-events` explains leader changes and summarises leadership per dimension.
 * Everything checked here is additive to the original contract, so the original fields are
 * asserted alongside to keep older clients covered.
 */
test.describe('Routing events (API)', () => {
  test('a leader change carries its reason and the scores that moved', async ({ api, merchant }) => {
    test.setTimeout(120_000)
    await seedLeaderFlip(api, merchant.id)
    const { body } = await waitForLeaderFlip(api)

    const flip = body.events.find(
      (event: any) => event.event_type === 'leader_changed' && event.gateway === FLIP_TO,
    )
    // The original contract.
    expect(flip).toMatchObject({
      merchant_id: merchant.id,
      payment_method_type: FLIP_DIMENSION.paymentMethodType,
      payment_method: FLIP_DIMENSION.paymentMethod,
      previous_gateway: FLIP_FROM,
    })
    expect(typeof flip.id).toBe('string')
    expect(flip.score).toBeGreaterThan(flip.previous_score)

    // stripe was still eligible, so adyen overtook it; stripe's score fell into the flip.
    expect(flip.leader_change_reason).toBe('overtaken')
    expect(flip.previous_score_before).toBeGreaterThan(flip.previous_score)
    expect(flip.score_before).toBeCloseTo(flip.score, 6)
  })

  test('the summary covers each dimension over the whole window', async ({ api, merchant }) => {
    test.setTimeout(120_000)
    await seedLeaderFlip(api, merchant.id)
    const { body } = await waitForLeaderFlip(api)

    expect(body.window_end_ms).toBeGreaterThan(body.window_start_ms)
    const dimension = body.dimensions.find(
      (entry: any) =>
        entry.payment_method_type === FLIP_DIMENSION.paymentMethodType &&
        entry.payment_method === FLIP_DIMENSION.paymentMethod,
    )
    expect(dimension).toBeTruthy()
    expect(dimension.leader).toBe(FLIP_TO)
    expect(dimension.leader_change_count).toBe(1)
    expect(dimension.leader_since_ms).toBeGreaterThanOrEqual(body.window_start_ms)
    // Nothing scored before the seed, so the window opened with no leader.
    expect(dimension.leader_at_window_start).toBeNull()

    // Leadership plus the stretch without fresh scores accounts for the whole window.
    const leaders = Object.fromEntries(dimension.leadership.map((entry: any) => [entry.gateway, entry.leader_ms]))
    expect(leaders[FLIP_FROM]).toBeGreaterThan(0)
    expect(leaders[FLIP_TO]).toBeGreaterThan(0)
    const accounted = Object.values<number>(leaders).reduce((sum, ms) => sum + ms, 0) + dimension.no_leader_ms
    expect(accounted).toBe(body.window_end_ms - body.window_start_ms)

    // The timeline tells the same story in order: no one, then stripe, then adyen until now.
    const timeline = dimension.timeline
    expect(timeline[0].start_ms).toBe(body.window_start_ms)
    expect(timeline[timeline.length - 1]).toMatchObject({ gateway: FLIP_TO, end_ms: body.window_end_ms })
    expect(timeline.map((span: any) => span.gateway)).toEqual([null, FLIP_FROM, FLIP_TO])
    for (let i = 1; i < timeline.length; i++) expect(timeline[i].start_ms).toBe(timeline[i - 1].end_ms)
    expect(timeline[2].start_ms).toBe(dimension.leader_since_ms)

    // Standings: adyen on top with a perfect score, stripe below it, both able to lead.
    expect(dimension.standings.map((standing: any) => standing.gateway)).toEqual([FLIP_TO, FLIP_FROM])
    expect(dimension.standings[0]).toMatchObject({ score: 1, eligible: true })
    expect(dimension.standings[1].score).toBeLessThan(1)
    expect(dimension.standings[1].eligible).toBe(true)

    // The trend: every PSP's success rate over time and the payments behind it, starting where
    // this payment method's traffic started.
    const trend = dimension.trend
    expect(trend.start_ms).toBeGreaterThanOrEqual(body.window_start_ms)
    expect(trend.payments.length).toBeLessThanOrEqual(120)
    expect(trend.payments.reduce((sum: number, n: number) => sum + n, 0)).toBeGreaterThan(0)
    const lines = Object.fromEntries(trend.gateways.map((g: any) => [g.gateway, g.scores]))
    expect(Object.keys(lines).sort()).toEqual([FLIP_FROM, FLIP_TO].sort())
    for (const scores of Object.values<any[]>(lines)) expect(scores).toHaveLength(trend.payments.length)
    // Adyen ends on top, as the standings say.
    const last = (scores: any[]) => scores.filter((v) => v != null).at(-1)
    expect(last(lines[FLIP_TO])).toBeGreaterThan(last(lines[FLIP_FROM]))
  })

  test('a merchant with no scores gets no summary', async ({ api, merchant }) => {
    const r = await api.raw('GET', '/analytics/routing-events', { qs: { range: '1h' } })
    expect(r.status).toBe(200)
    expect(r.body).toMatchObject({ merchant_id: merchant.id, events: [] })
    // Omitted rather than sent empty, like the other optional fields.
    expect(r.body.dimensions).toBeUndefined()
  })
})
