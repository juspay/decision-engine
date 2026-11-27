import { expect } from '@playwright/test'
import factory from '../fixtures/factory'
import type { ApiClient } from '../fixtures/api-client'
import { poll } from './poll'

/**
 * A deterministic SR leader change for the routing-events feed, through the real
 * `/decide-gateway` → `/update-gateway-score` path:
 *
 *   1. stripe takes 12 successes and leads (score 1.0).
 *   2. adyen takes 12 successes and ties it — a tie never flips the lead.
 *   3. stripe takes failures: its score falls below adyen's and adyen takes over.
 *
 * Each payment is routed with a single eligible gateway so the pick is fixed. The phases
 * sit a couple of seconds apart because the 1-hour feed scans 1-second buckets: payments
 * landing in the same second collapse into one bucket and would hide the flip.
 */
export const FLIP_DIMENSION = { paymentMethodType: 'CREDIT', paymentMethod: 'CARD' }
export const [FLIP_FROM, FLIP_TO] = [
  factory.gatewayConnector('stripe').gateway_name,
  factory.gatewayConnector('adyen').gateway_name,
]

async function route(api: ApiClient, merchantId: string, gateway: string, status: 'AUTHORIZED' | 'FAILURE') {
  const paymentId = factory.paymentId('routing_events')
  const decision = await api.decideGateway(
    factory.srDecideGatewayRequest({
      merchantId,
      eligibleGatewayList: [gateway],
      paymentInfo: { ...FLIP_DIMENSION, paymentId },
    }),
  )
  expect(decision.body.decided_gateway).toBe(gateway)
  await api.updateGatewayScore(
    factory.updateGatewayScoreRequest({ merchantId, gateway, paymentId, status }),
  )
}

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

export async function seedLeaderFlip(api: ApiClient, merchantId: string) {
  await api.createSuccessRateConfig(merchantId, { defaultBucketSize: 20, defaultHedgingPercent: 0 })
  for (let i = 0; i < 12; i++) await route(api, merchantId, FLIP_FROM, 'AUTHORIZED')
  await pause(2_000)
  for (let i = 0; i < 12; i++) await route(api, merchantId, FLIP_TO, 'AUTHORIZED')
  await pause(2_000)
  for (let i = 0; i < 6; i++) await route(api, merchantId, FLIP_FROM, 'FAILURE')
}

/** The 1-hour routing-events response once the flip has landed in ClickHouse. */
export async function waitForLeaderFlip(api: ApiClient) {
  return poll(
    () =>
      api.raw('GET', '/analytics/routing-events', {
        failOnStatusCode: false,
        qs: { range: '1h', bucket: '1s' },
      }),
    ({ status, body }) =>
      status === 200 &&
      (body?.events ?? []).some(
        (event: any) =>
          event.event_type === 'leader_changed' &&
          event.previous_gateway === FLIP_FROM &&
          event.gateway === FLIP_TO,
      ),
    { message: `Expected a ${FLIP_FROM} → ${FLIP_TO} leader change`, timeout: 60_000 },
  )
}
