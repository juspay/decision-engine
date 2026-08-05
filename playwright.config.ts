import { defineConfig, devices } from '@playwright/test'

/**
 * Playwright config for Decision Engine e2e.
 *
 * The runtime stack (API, dashboard UI, Postgres/Redis/Kafka/ClickHouse) is booted
 * OUT of band by `cypress/scripts/run-e2e.js` (source or docker mode), which passes the
 * resolved URLs in as plain env vars. There is deliberately no `webServer` block here.
 *
 * Two projects:
 *   - `api` — no browser; API-contract specs in tests/api (baseURL = decision-engine API).
 *   - `ui`  — chromium; user-journey specs in tests/e2e (baseURL = dashboard UI).
 *
 * Local: `node cypress/scripts/run-e2e.js source` (E2E_RUNNER defaults to playwright),
 * or against an already-up stack: `npx playwright test --project=api`.
 */

const API_BASE_URL = process.env.API_BASE_URL || 'http://localhost:8080'
const UI_BASE_URL = process.env.UI_BASE_URL || 'http://localhost:5173'

export default defineConfig({
  testDir: 'tests',
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  // Cap workers in CI so the shared runtime stack isn't overwhelmed; unbounded locally.
  // The CI runner hosts the API, dashboard, Postgres, Redis, Kafka and ClickHouse alongside the tests
  // on the same few vCPUs, so this is conservative by default. Override with PW_WORKERS once a full
  // run has been timed — tune from the measurement, not a guess.
  workers: Number(process.env.PW_WORKERS) || (process.env.CI ? 2 : undefined),
  reporter: [['list'], ['html', { open: 'never' }]],
  timeout: 60_000,
  expect: { timeout: 10_000 },
  use: {
    // Sent to APIRequestContext and page.goto for relative URLs. Overridden per-project.
    baseURL: API_BASE_URL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
    extraHTTPHeaders: {
      'x-tenant-id': 'public',
    },
  },
  projects: [
    {
      name: 'api',
      testDir: 'tests/api',
      use: {
        baseURL: API_BASE_URL,
      },
    },
    {
      name: 'ui',
      testDir: 'tests/e2e',
      use: {
        ...devices['Desktop Chrome'],
        baseURL: UI_BASE_URL,
        viewport: { width: 1280, height: 720 },
      },
    },
  ],
})
