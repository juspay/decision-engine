import { test, expect } from '../../fixtures/test'
import { EXPECTED_CLICKHOUSE_TABLES, existingTables } from '../../helpers/clickhouse'

/**
 * Port of cypress/e2e/runtime/runtime-smoke.cy.js.
 *
 * Lives in the `api` project rather than `tests/e2e/` — despite the Cypress original sitting under
 * e2e/, it never opens a browser. It checks the surfaces AROUND the app: the docs site is serving, and
 * ClickHouse has the analytics schema the ingestion pipeline writes into.
 *
 * The ClickHouse assertion earns its place: a missing table there does not fail any API call
 * synchronously — analytics endpoints just return empty forever — so without this the failure mode is
 * "the dashboard is mysteriously blank" rather than a test failure.
 *
 * The docs tests skip when DOCS_BASE_URL is unset, so the spec still passes against a hand-started
 * stack that didn't boot the docs site.
 */

const DOCS_BASE_URL = process.env.DOCS_BASE_URL

test.describe('Runtime surface smoke', () => {
  test('ClickHouse has the analytics tables the pipeline writes to', async ({ request }) => {
    const found = await existingTables(request, EXPECTED_CLICKHOUSE_TABLES)

    for (const table of EXPECTED_CLICKHOUSE_TABLES) {
      expect(found.has(table), `ClickHouse table ${table} should exist`).toBe(true)
    }
  })

  test('the docs site serves its landing and API reference pages', async ({ request }) => {
    test.skip(!DOCS_BASE_URL, 'DOCS_BASE_URL not set — docs site not part of this run')

    for (const path of ['/introduction', '/api-reference']) {
      const response = await request.get(`${DOCS_BASE_URL}${path}`)
      expect(response.status(), `${path} should serve`).toBe(200)
      expect(await response.text()).toContain('Decision Engine')
    }
  })

  test('the docs API reference includes the health check endpoint', async ({ request }) => {
    test.skip(!DOCS_BASE_URL, 'DOCS_BASE_URL not set — docs site not part of this run')

    const response = await request.get(`${DOCS_BASE_URL}/api-reference/endpoint/healthCheck`)
    expect(response.status()).toBe(200)
    expect((await response.text()).toLowerCase()).toContain('health')
  })

  test('the runtime reports which mode it booted in', async () => {
    const mode = process.env.RUNTIME_MODE
    test.skip(!mode, 'RUNTIME_MODE not set — stack was not booted by run-e2e.js')

    expect(['source', 'docker', 'manual']).toContain(mode)
  })
})
