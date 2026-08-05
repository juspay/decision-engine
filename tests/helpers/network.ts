import type { Page } from '@playwright/test'

/**
 * Port of the Cypress `cy.intercept(...).as('x')` + `cy.wait('@x')` pattern, which the rule-creation
 * specs use ~15 times — every one of them asserting on BOTH the response status and the request body
 * the UI built. Playwright exposes both off a single `Response`, so one helper covers the pattern.
 *
 * ORDERING MATTERS: create the promise BEFORE the action that triggers the request, then await it
 * after. A `waitForResponse` registered after the click has already missed it.
 *
 *   const created = expectApiCall(page, '/routing/create')
 *   await page.getByRole('button', { name: 'Create Rule' }).click()
 *   const { status, requestBody } = await created
 *
 * Matches on a PATH SUFFIX rather than a full URL: the dashboard calls the API under
 * `/decision-engine-api` in dev and `/decision-engine/api` in production builds, so anchoring on the
 * prefix would break in one mode or the other.
 */

export interface ApiCall<T = any> {
  status: number
  /** The JSON body the UI sent, or null for GETs / non-JSON requests. */
  requestBody: any
  /** The parsed JSON response, or null when the response wasn't JSON. */
  body: T
}

export async function expectApiCall<T = any>(
  page: Page,
  pathSuffix: string,
  method = 'POST',
  options: { timeout?: number } = {},
): Promise<ApiCall<T>> {
  const response = await page.waitForResponse(
    (r) => r.url().includes(pathSuffix) && r.request().method() === method,
    { timeout: options.timeout ?? 20_000 },
  )

  let requestBody: any = null
  try {
    requestBody = response.request().postDataJSON()
  } catch {
    // GET, or a non-JSON body — leave null.
  }

  let body: any = null
  try {
    body = await response.json()
  } catch {
    // Empty or non-JSON response (e.g. /routing/deactivate returns no body).
  }

  return { status: response.status(), requestBody, body }
}
