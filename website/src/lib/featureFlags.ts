export const FEATURE_FLAGS = {
  GSM_RETRY_IN_SIMULATION: false,
  SMART_RETRY_IN_ANALYTICS: false,
  SMART_RETRY_IN_SIMULATION: false,
  // Rule deletion is hidden for parity with the Hyperswitch dashboard, which cannot
  // delete routing rules yet (juspay/hyperswitch-control-center#5495, item 4).
  // Flip together with the commented-out /routing/delete route in src/app.rs.
  RULE_DELETION: false,
}
