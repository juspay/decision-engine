import type { ApiClient } from '../fixtures/api-client'
import factory from '../fixtures/factory'

/**
 * Shared builders for the A/B specs.
 *
 * An experiment is an ordinary routing algorithm — `StaticRoutingAlgorithm::AbTest` in the
 * merchant's PAYMENT activation slot — so it goes through /routing/create and /routing/activate
 * like any rule. These wrap the payload shape so a change to it lands in one place.
 */

/** Mirrors the backend `ArmStrategy`. `current` is resolved server-side at create time. */
export type Arm =
  | { kind: 'current' }
  | { kind: 'rule'; algorithm_id: string }
  | { kind: 'sr'; sr_config?: Record<string, unknown> }
  | { kind: 'hybrid'; algorithm_id: string; sr_config?: Record<string, unknown> }

export function experimentPayload(
  merchantId: string,
  control: Arm,
  variant: Arm,
  overrides: Record<string, unknown> = {},
) {
  return {
    name: factory.ruleName('abtest'),
    description: 'playwright a/b experiment',
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: {
      type: 'ab_test',
      data: {
        control,
        variant,
        variant_split_pct: 10,
        min_sample_size: 100,
        guardrail_threshold_pp: 3,
        ...overrides,
      },
    },
  }
}

/** Create a saved single-connector rule and return its id, without activating it. */
export async function makeRule(api: ApiClient, merchantId: string, gateway = 'stripe'): Promise<string> {
  const created = await api.createRoutingAlgorithm(
    factory.singleRoutingPayload(merchantId, { name: factory.ruleName('ab_leg'), gateway }),
  )
  return created.body.rule_id as string
}

/** The merchant's stored experiment, read back through the active-algorithm listing. */
export async function readActiveExperiment(api: ApiClient, merchantId: string): Promise<any> {
  const r = await api.listActiveRoutingAlgorithms(merchantId)
  const rows = Array.isArray(r.body) ? r.body : r.body?.data ?? []
  const row = rows.find((a: any) => (a.algorithm_data ?? a.algorithm)?.type === 'ab_test')
  return (row?.algorithm_data ?? row?.algorithm)?.data
}

/** Turn a merchant feature on or off by slug. */
export function setFeature(api: ApiClient, merchantId: string, slug: string, enabled: boolean) {
  return api.raw('POST', `/merchant-account/${merchantId}/features/${slug}`, {
    body: { enabled },
    failOnStatusCode: false,
  })
}

/**
 * Autopilot is two flags, and `sr_auto_calibration.rs` calibrates only merchants that have both.
 * A test that flips one and expects a control arm to notice is testing nothing.
 */
export async function setAutopilot(api: ApiClient, merchantId: string, enabled: boolean) {
  await setFeature(api, merchantId, 'autopilot', enabled)
  await setFeature(api, merchantId, 'auto-calibration', enabled)
}

/**
 * Interception at /decide-gateway is gated on `ab-test-real-payments`, deliberately separate from
 * "the experiment is activated": building an experiment and pointing real money at it should not
 * be the same click.
 */
export function enableRealPayments(api: ApiClient, merchantId: string) {
  return api.raw('POST', `/merchant-account/${merchantId}/features/ab-test-real-payments`, {
    body: { enabled: true },
    failOnStatusCode: false,
  })
}
