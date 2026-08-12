import { defineConfig, devices } from '@playwright/test'
import { SUPER_ADMIN_EMAIL } from './tests/fixtures/super-admin'

/**
 * Playwright config for Decision Engine e2e.
 *
 * Datastores (Postgres/Redis/Kafka/ClickHouse) are brought up out of band — by `docker compose` in
 * CI, or by `cypress/scripts/run-e2e.js` locally. The two APPLICATION processes are started by the
 * `webServer` block below, so Playwright owns their readiness and teardown.
 *
 * `reuseExistingServer` is on outside CI: if you already have the API and dashboard running, Playwright
 * attaches to them instead of starting its own. Set PW_NO_WEBSERVER=1 to opt out entirely (e.g. when
 * run-e2e.js has already booted everything).
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

/**
 * Start the app processes ourselves unless something else already owns them — `run-e2e.js` boots the
 * whole stack via oneclick.sh, so it sets PW_NO_WEBSERVER to stay in charge.
 */
const webServer = process.env.PW_NO_WEBSERVER
  ? undefined
  : [
      {
        // Built by CI's `cargo build --no-default-features --features postgres` step.
        command: './target/debug/open_router',
        url: `${API_BASE_URL}/health`,
        reuseExistingServer: !process.env.CI,
        // A debug build on a loaded runner needs well over the 60s default to finish booting.
        timeout: 180_000,
        stdout: 'pipe' as const,
        stderr: 'pipe' as const,
        // Inject the super-admin roster for the super-admin-view spec, so the test identity lives
        // with the tests instead of in shipped config. Config parses this comma-separated list via
        // `.with_list_parse_key("user_auth.super_admin_emails")` (src/config.rs). Merged over
        // process.env, so CI's other DECISION_ENGINE__ overrides still apply.
        env: {
          ...(process.env as Record<string, string>),
          DECISION_ENGINE__USER_AUTH__SUPER_ADMIN_EMAILS: SUPER_ADMIN_EMAIL,
        },
      },
      {
        command: 'npm --prefix website run dev',
        url: UI_BASE_URL,
        reuseExistingServer: !process.env.CI,
        timeout: 180_000,
        stdout: 'pipe' as const,
        stderr: 'pipe' as const,
      },
    ]

export default defineConfig({
  testDir: 'tests',
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: Number(process.env.PW_WORKERS) || (process.env.CI ? 1 : undefined),
  reporter: [['list'], ['html', { open: 'never' }]],
  webServer,
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
