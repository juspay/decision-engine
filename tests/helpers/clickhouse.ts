import type { APIRequestContext } from '@playwright/test'

/**
 * Port of the Cypress `clickhouseQuery` task (cypress.config.js). Playwright has no plugin process, so
 * the query goes over ClickHouse's HTTP interface using the test's own request context.
 *
 * `run-e2e.js` passes CLICKHOUSE_HTTP_URL / _DATABASE / _USER / _PASSWORD through to Playwright, so
 * these defaults only apply when running against a hand-started stack.
 */

const HTTP_URL = process.env.CLICKHOUSE_HTTP_URL || 'http://localhost:8123'
const DATABASE = process.env.CLICKHOUSE_DATABASE || 'default'
const USER = process.env.CLICKHOUSE_USER || 'decision_engine'
const PASSWORD = process.env.CLICKHOUSE_PASSWORD || 'decision_engine'

/** Tables the analytics pipeline needs, mirroring EXPECTED_CLICKHOUSE_TABLES in cypress.config.js. */
export const EXPECTED_CLICKHOUSE_TABLES = [
  'analytics_api_events_queue',
  'analytics_domain_events_queue',
  'analytics_api_events',
  'analytics_domain_events',
  'analytics_payment_audit_summary_buckets',
  'analytics_payment_audit_lookup_summaries',
]

/** Run a query and return the raw response text (use `FORMAT TSV` for line-parseable output). */
export async function clickhouseQuery(request: APIRequestContext, query: string): Promise<string> {
  const url = new URL(HTTP_URL)
  url.searchParams.set('database', DATABASE)
  url.searchParams.set('query', query)

  const response = await request.get(url.toString(), {
    headers: {
      Authorization: `Basic ${Buffer.from(`${USER}:${PASSWORD}`).toString('base64')}`,
    },
  })

  const body = await response.text()
  if (!response.ok()) {
    throw new Error(`ClickHouse query failed (${response.status()}): ${body}`)
  }
  return body
}

/** Names of the given tables that actually exist in the current database. */
export async function existingTables(
  request: APIRequestContext,
  tables: string[],
): Promise<Set<string>> {
  const quoted = tables.map((t) => `'${t}'`).join(', ')
  const raw = await clickhouseQuery(
    request,
    `SELECT name FROM system.tables WHERE database = currentDatabase() AND name IN (${quoted}) ORDER BY name FORMAT TSV`,
  )
  return new Set(
    raw
      .split('\n')
      .map((v) => v.trim())
      .filter(Boolean),
  )
}
