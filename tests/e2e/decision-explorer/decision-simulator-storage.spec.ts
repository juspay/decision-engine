import { test, expect } from '../../fixtures/test'

/**
 * Regression coverage for the localStorage crash fixed in PR #438 (DecisionSimulatorPage's
 * "decision-explorer-state-v2" persistence) and the resume-index bug found in review on that
 * same PR.
 *
 * Pausing a long simulation persists its run history to localStorage so "Resume" can pick it
 * back up later. A long enough run's full history used to exceed the browser's per-origin quota
 * and crash the page (QuotaExceededError thrown inside a useEffect). The fix caps what's
 * persisted to the last 200 rows and sweeps expired entries for every merchant scope, not just
 * the active one — but that same cap made the resume index wrong if it was ever derived from the
 * capped array's length instead of an absolute count, letting a later resume replay
 * already-completed transactions against the real API. These tests drive the real page against a
 * real backend and manipulate real localStorage rather than a mock, since the bug only exists at
 * that boundary.
 */
test.describe('Decision Simulator — persisted state cap, sweep, and resume integrity', () => {
  test('an over-cap history is trimmed on the next save, and other scopes are swept regardless of expiry cause', async ({ authedPage }) => {
    await authedPage.goto('/decisions/simulator')
    await expect(authedPage.getByRole('heading', { name: 'Decision Simulator' })).toBeVisible()
    await authedPage.waitForTimeout(500) // let the persist effect write its first snapshot

    const key = await authedPage.evaluate(() =>
      Object.keys(localStorage).find(k => k.startsWith('decision-explorer-state-v2:')) ?? null,
    )
    expect(key, 'the app should have written its explorer-state key by now').toBeTruthy()
    if (!key) throw new Error('unreachable — asserted above')

    // 3,000 rows lands fine under a real browser's quota but is far past the app's own 200-row
    // persist cap — the actual "long run, one prior successful save" scenario the fix targets.
    const landedCount = await authedPage.evaluate(({ k }) => {
      const parsed = JSON.parse(localStorage.getItem(k) || '{}')
      const row = { decidedGateway: 'stripe', status: 'CHARGED', timestamp: new Date().toISOString(), routingApproach: null, gatewayPriorityMap: null, amount: 100, currency: 'USD' }
      const rows = Array.from({ length: 3000 }, (_, i) => ({ ...row, paymentId: `sim_${i}` }))
      localStorage.setItem(k, JSON.stringify({ ...parsed, simulationResults: rows, resultDataUpdatedAtMs: Date.now() }))
      return JSON.parse(localStorage.getItem(k)!).simulationResults.length
    }, { k: key })
    expect(landedCount, 'the 3,000-row seed should land in real localStorage without throwing').toBe(3000)

    // Two other scopes the sweep must reach, since it walks every stored scope, not the active
    // one: an expired entry and one that fails to parse as JSON altogether.
    const staleKey = 'decision-explorer-state-v2:regression-stale%3Aold-merchant'
    const corruptKey = 'decision-explorer-state-v2:regression-corrupt%3Aold-merchant'
    await authedPage.evaluate(({ sk, ck }) => {
      localStorage.setItem(sk, JSON.stringify({ resultDataUpdatedAtMs: Date.now() - 20 * 60 * 1000 }))
      localStorage.setItem(ck, '{not valid json')
    }, { sk: staleKey, ck: corruptKey })

    // Reload: forces loadExplorerState to restore the 3,000-row snapshot, and the mount effect
    // to run sweepExpiredExplorerState fresh across every scope.
    await authedPage.reload()
    await expect(authedPage.getByRole('heading', { name: 'Decision Simulator' })).toBeVisible()
    await expect(authedPage.getByText('Dashboard Error')).toHaveCount(0)
    await authedPage.waitForTimeout(500) // let the post-reload persist effect fire at least once

    const afterRaw = await authedPage.evaluate((k) => localStorage.getItem(k), key)
    const afterParsed = JSON.parse(afterRaw!)
    expect(afterParsed.simulationResults.length, 'the next natural save should cap the restored 3,000 rows back down to 200').toBeLessThanOrEqual(200)
    expect(await authedPage.evaluate((sk) => localStorage.getItem(sk), staleKey), 'the sweep should evict the expired other-scope entry').toBeNull()
    expect(await authedPage.evaluate((ck) => localStorage.getItem(ck), corruptKey), 'the sweep should evict the entry that fails to parse').toBeNull()
  })

  test('a write that itself exceeds quota degrades gracefully instead of crashing the page', async ({ authedPage }) => {
    await authedPage.goto('/decisions/simulator')
    await expect(authedPage.getByRole('heading', { name: 'Decision Simulator' })).toBeVisible()
    await authedPage.waitForTimeout(500)

    const key = await authedPage.evaluate(() =>
      Object.keys(localStorage).find(k => k.startsWith('decision-explorer-state-v2:')) ?? null,
    )
    expect(key).toBeTruthy()
    if (!key) throw new Error('unreachable — asserted above')

    // 60,000 rows is large enough to exceed real quota on the write itself — the exact original
    // crash condition, reproduced with real numbers rather than assumed.
    const seedResult = await authedPage.evaluate(({ k }) => {
      const parsed = JSON.parse(localStorage.getItem(k) || '{}')
      const row = { decidedGateway: 'stripe', status: 'CHARGED', timestamp: new Date().toISOString(), routingApproach: null, gatewayPriorityMap: null, amount: 100, currency: 'USD' }
      const huge = Array.from({ length: 60000 }, (_, i) => ({ ...row, paymentId: `sim_${i}` }))
      try {
        localStorage.setItem(k, JSON.stringify({ ...parsed, simulationResults: huge, resultDataUpdatedAtMs: Date.now() }))
        return 'ok'
      } catch {
        return 'quota-exceeded'
      }
    }, { k: key })
    expect(seedResult, '60,000 rows should genuinely exceed real quota — confirms this scenario is realistic, not assumed').toBe('quota-exceeded')

    await authedPage.reload()
    await expect(authedPage.getByRole('heading', { name: 'Decision Simulator' })).toBeVisible()
    await expect(authedPage.getByText('Dashboard Error')).toHaveCount(0)
  })

  test('resuming a run whose persisted history is shorter than its true index does not replay completed transactions', async ({ api, authedPage, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const dispatchedIndices: number[] = []
    authedPage.on('request', (req) => {
      if (!req.url().includes('/decide-gateway')) return
      const m = /"paymentId":"sim_\d+_(\d+)"/.exec(req.postData() || '')
      if (m) dispatchedIndices.push(Number(m[1]))
    })

    await authedPage.goto('/decisions/simulator')
    await expect(authedPage.getByRole('heading', { name: 'Decision Simulator' })).toBeVisible()

    await authedPage.getByRole('button', { name: 'Run simulation' }).click()
    await expect(authedPage.getByRole('button', { name: 'Pause', exact: true })).toBeVisible({ timeout: 15_000 })
    await expect.poll(() => dispatchedIndices.length, { timeout: 20_000, intervals: [300] }).toBeGreaterThanOrEqual(5)
    await authedPage.getByRole('button', { name: 'Pause', exact: true }).click()
    await authedPage.waitForTimeout(500) // let the paused flush land

    const preReloadDispatched = [...dispatchedIndices]
    const key = await authedPage.evaluate(() =>
      Object.keys(localStorage).find(k => k.startsWith('decision-explorer-state-v2:')) ?? null,
    )
    expect(key, 'expected a persisted snapshot to exist after pausing').toBeTruthy()
    if (!key) throw new Error('unreachable — asserted above')

    const beforeTamper = await authedPage.evaluate((k) => JSON.parse(localStorage.getItem(k)!), key)
    const trueNextIndex = beforeTamper.resumableRun?.nextIndex
    expect(trueNextIndex, 'expected a resumable snapshot with a real nextIndex').toBeGreaterThan(0)

    // Force the exact mismatch a long run's 200-row cap produces naturally — rows far shorter
    // than the true index — without waiting on 200+ real transactions.
    await authedPage.evaluate(({ k, state }) => {
      state.simulationResults = state.simulationResults.slice(-1)
      localStorage.setItem(k, JSON.stringify(state))
    }, { k: key, state: beforeTamper })

    await authedPage.reload()
    await expect(authedPage.getByRole('heading', { name: 'Decision Simulator' })).toBeVisible()
    await expect(authedPage.getByText('Dashboard Error')).toHaveCount(0)
    const resumeButton = authedPage.getByRole('button', { name: 'Resume run' })
    await expect(resumeButton).toBeVisible()
    // The button's own tooltip must read the true index, not the truncated array length.
    await expect(resumeButton).toHaveAttribute('title', new RegExp(`transaction ${trueNextIndex + 1} of`))

    dispatchedIndices.length = 0 // isolate what the resumed run dispatches
    await resumeButton.click()
    await expect.poll(() => dispatchedIndices.length, { timeout: 20_000, intervals: [300] }).toBeGreaterThanOrEqual(3)
    await authedPage.waitForTimeout(500)

    const postResumeDispatched = [...dispatchedIndices]
    const overlap = postResumeDispatched.filter((i) => preReloadDispatched.includes(i))
    expect(overlap, 'resumed dispatch must not repeat a payment index sent before the reload').toEqual([])
    expect(Math.min(...postResumeDispatched), 'the first resumed index must be at/after the true persisted nextIndex').toBeGreaterThanOrEqual(trueNextIndex)

    // The actual bug lived here: a SECOND pause, after a resume that started from a truncated
    // in-memory `results` array. The old code persisted results.length (small, wrong); the fix
    // persists an absolute counter seeded from the true resume point.
    await authedPage.getByRole('button', { name: 'Pause', exact: true }).click()
    await authedPage.waitForTimeout(500)
    const secondPauseState = await authedPage.evaluate((k) => JSON.parse(localStorage.getItem(k)!), key)
    expect(
      secondPauseState.resumableRun.nextIndex,
      'the persisted resume index after a second pause must not regress to the (still-truncated) in-memory row count',
    ).not.toBe(secondPauseState.simulationResults.length)
    expect(
      secondPauseState.resumableRun.nextIndex,
      'the persisted resume index must be at least the true pre-resume count plus what was dispatched this run',
    ).toBeGreaterThanOrEqual(trueNextIndex + postResumeDispatched.length - 5)

    await authedPage.getByRole('button', { name: 'Stop', exact: true }).click()
  })
})
