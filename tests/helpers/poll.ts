/**
 * Port of Cypress `cy.pollRequest` (cypress/support/commands.js): re-run a request until a predicate
 * passes or the timeout elapses.
 *
 * Prefer Playwright's built-in `expect.poll()` for plain "eventually true" assertions. Reach for this
 * when the SETTLED RESPONSE is needed afterwards — `expect.poll` returns a matcher, not the value, and
 * most analytics assertions need to read the body they waited for.
 *
 * The `{ status, body }` constraint is what lets a timeout print the last response it saw, which is
 * the single most useful thing this helper does when ClickHouse ingestion is lagging behind the test.
 */

export interface PollOptions {
  /** Included in the thrown error, followed by the last observed status + body. */
  message: string
  /** Give up after this long. Default 30s. */
  timeout?: number
  /** Wait between attempts. Default 2s. */
  interval?: number
}

export async function poll<T extends { status: number; body: any }>(
  request: () => Promise<T>,
  predicate: (result: T) => boolean,
  options: PollOptions,
): Promise<T> {
  const { message, timeout = 30_000, interval = 2_000 } = options
  const startedAt = Date.now()
  let last: T | null = null

  for (;;) {
    const result = await request()
    last = result
    if (predicate(result)) return result

    if (Date.now() - startedAt >= timeout) {
      const context = last
        ? ` Last result: ${JSON.stringify({ status: last.status, response: last.body }).slice(0, 1000)}`
        : ''
      throw new Error(`${message}.${context}`)
    }

    await new Promise((resolve) => setTimeout(resolve, interval))
  }
}
