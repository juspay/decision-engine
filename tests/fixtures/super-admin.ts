/**
 * The platform super-admin identity used by the super-admin-view spec.
 *
 * The roster is config-only by design (there's no runtime API to grant super-admin), so a test of
 * the positive path needs one known identity in the loaded roster. Rather than shipping it in
 * config/development.toml, playwright.config.ts injects this email into the API server it starts via
 * `DECISION_ENGINE__USER_AUTH__SUPER_ADMIN_EMAILS` — so the test identity lives with the tests.
 *
 * Single source of truth: both playwright.config.ts (the injected roster) and the spec (the login it
 * performs) import from here, so they can never drift apart.
 */
export const SUPER_ADMIN_EMAIL = 'superadmin@example.com'
export const SUPER_ADMIN_PASSWORD = 'Password123!'
