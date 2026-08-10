import type { Page } from '@playwright/test'

/**
 * Stub an API response. Globs are prefix-agnostic (`**\/auth/signup`) because the dashboard calls
 * the API under `/decision-engine-api` in dev and `/decision-engine/api` in production builds.
 *
 * Register before `page.goto` — a route added after navigation misses in-flight requests. Pass
 * `method` when the path serves more than one verb; non-matching requests hit the real server.
 */
export async function stubApi(
  page: Page,
  pathGlob: string,
  response: { status: number; body: unknown },
  method?: string,
): Promise<void> {
  await page.route(pathGlob, (route) => {
    if (method && route.request().method().toUpperCase() !== method.toUpperCase()) {
      return route.continue()
    }
    return route.fulfill({
      status: response.status,
      contentType: 'application/json',
      body: JSON.stringify(response.body),
    })
  })
}
