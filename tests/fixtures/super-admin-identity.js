/**
 * The platform super-admin test identity. Plain CommonJS so scripts/run-e2e.js can load it too;
 * typed specs and playwright.config.ts import it through ./super-admin.ts.
 */
module.exports = {
  SUPER_ADMIN_EMAIL: 'superadmin@example.com',
  SUPER_ADMIN_PASSWORD: 'Password123!',
}
