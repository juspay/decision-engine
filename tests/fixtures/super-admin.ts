/**
 * The platform super-admin identity used by the super-admin-view spec.
 *
 * The roster is config-only by design (there's no runtime API to grant super-admin), so a test of
 * the positive path needs one known identity in the loaded roster. Rather than shipping it in
 * config/development.toml, the test runners inject this email into the API server they start via
 * `DECISION_ENGINE__USER_AUTH__SUPER_ADMIN_EMAILS` — so the test identity lives with the tests.
 *
 * Single source of truth: `super-admin-identity.js`, which playwright.config.ts (the API it starts),
 * scripts/run-e2e.js (the API oneclick.sh starts) and the spec (the login it performs) all read, so
 * they can never drift apart.
 */
import identity from './super-admin-identity.js'

export const SUPER_ADMIN_EMAIL: string = identity.SUPER_ADMIN_EMAIL
export const SUPER_ADMIN_PASSWORD: string = identity.SUPER_ADMIN_PASSWORD
