import type { Page } from '@playwright/test'

/**
 * Ports of the two `cy.intercept` STUBBING shapes the UI specs use (as opposed to the
 * observe-and-assert shape, which lives in network.ts).
 *
 * Globs here are deliberately prefix-agnostic (`**\/auth/signup`, not the Cypress
 * `**\/decision-engine-api/auth/signup`): the dashboard calls the API under `/decision-engine-api` in
 * dev and `/decision-engine/api` in production builds. Playwright's `**` matches `/`, so the short
 * form covers both.
 *
 * Register these BEFORE `page.goto` — a route added after navigation misses in-flight requests.
 */

/** Replace a response outright. Port of `cy.intercept(method, url, { statusCode, body })`. */
export async function stubApi(
  page: Page,
  pathGlob: string,
  response: { status: number; body: unknown },
): Promise<void> {
  await page.route(pathGlob, (route) =>
    route.fulfill({
      status: response.status,
      contentType: 'application/json',
      body: JSON.stringify(response.body),
    }),
  )
}

/**
 * Slow a real request down so a transient loading state stays observable. Port of
 * `cy.intercept(url, req => req.continue(res => res.setDelay(ms)))`.
 *
 * Note the semantic difference: Cypress delays the RESPONSE, this delays the REQUEST. Equivalent for
 * observing a loading indicator (the UI is waiting either way), but not identical.
 */
export async function delayApi(page: Page, pathGlob: string, ms: number): Promise<void> {
  await page.route(pathGlob, async (route) => {
    await new Promise((resolve) => setTimeout(resolve, ms))
    await route.continue()
  })
}
