import { test, expect } from '../../fixtures/test'

/**
 * Every dashboard route that has no dedicated spec, checked for the one failure that matters most: does
 * the page render at all for an authenticated user, or does it throw and hit the ErrorBoundary?
 *
 * This is deliberately shallow. These pages have deep functionality that belongs in their own specs;
 * what this catches is the class of regression that takes a whole page down — a bad import, a null
 * dereference on empty data, a route that silently redirects. That failure is currently invisible in CI.
 *
 * Table-driven on purpose: adding a page should be a one-line diff. Runs on the worker-scoped merchant
 * since nothing here mutates state.
 *
 * `/` is NOT listed — dashboard-overview.spec.ts covers Overview properly.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

const PAGES = [
  { path: '/routing', heading: 'Routing Hub' },
  { path: '/routing/sr', heading: 'Multi Objective Routing' },
  { path: '/routing/ab-testing', heading: 'A/B Testing' },
  { path: '/decisions/simulator', heading: 'Decision Simulator' },
  { path: '/events', heading: 'Routing events' },
  { path: '/members', heading: 'Members' },
  { path: '/account', heading: 'Account' },
]

test.describe('Dashboard route smoke', () => {
  for (const { path, heading } of PAGES) {
    test(`${path} renders without crashing`, async ({ sharedPage }) => {
      const pageErrors: Error[] = []
      sharedPage.on('pageerror', (error) => pageErrors.push(error))

      await sharedPage.goto(path)

      await expect(sharedPage.getByRole('heading', { level: 1, name: heading })).toBeVisible({
        timeout: 20_000,
      })
      // The ErrorBoundary fallback — if this is present the page threw during render.
      await expect(sharedPage.getByText('Dashboard Error')).toHaveCount(0)
      expect(pageErrors, `${path} raised uncaught errors: ${pageErrors.map((e) => e.message).join('; ')}`)
        .toHaveLength(0)
    })
  }

  test('an unknown dashboard route redirects rather than 404ing', async ({ sharedPage }) => {
    await sharedPage.goto('/this-route-does-not-exist')

    // The catch-all sends the user back to Overview instead of a dead end.
    await expect(sharedPage.getByRole('heading', { level: 1, name: 'Overview' })).toBeVisible({
      timeout: 20_000,
    })
  })

  test('the legacy cost route still resolves for old bookmarks', async ({ sharedPage }) => {
    await sharedPage.goto('/routing/cost')

    // Cost estimation moved into Multi Objective Routing as a tab; the old path must keep working.
    await expect(sharedPage).toHaveURL(/\/routing\/sr\?tab=cost/)
  })
})
