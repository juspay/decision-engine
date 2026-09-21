import { PaymentAuditEvent } from '../types/api'

/**
 * How an audit event is named in the UI. Decision Audit, Decision Explorer and Decision Simulator
 * all render the same `/analytics/payment-audit` events, so the naming lives here once — a new
 * flow type is labelled in one place and shows up the same on all three.
 */

export function humanizeAuditValue(value?: string | null) {
  if (!value) return ''
  const normalized = value
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase()

  return normalized.replace(/\b\w/g, (char) => char.toUpperCase())
}

export function flowTypeValue(event: PaymentAuditEvent) {
  return event.flow_type || ''
}

export function isErrorFlow(flowType: string) {
  return flowType.endsWith('_error')
}

export function isPreviewFlow(flowType: string) {
  return flowType.startsWith('routing_evaluate_') && flowType !== 'routing_evaluate_request_hit'
}

export function isRuleHitFlow(flowType: string) {
  return flowType === 'decide_gateway_rule_hit'
}

export function isUpdateFlow(flowType: string) {
  return flowType.startsWith('update_gateway_score_') || flowType.startsWith('update_score_legacy_')
}

export function isHybridFlow(flowType: string) {
  return flowType.startsWith('routing_hybrid_')
}

export function isDecisionFlow(flowType: string) {
  return (
    (flowType.startsWith('decide_gateway_') || isHybridFlow(flowType)) && !isRuleHitFlow(flowType)
  )
}

export function routeLabel(route?: string | null) {
  if (!route) return 'Unknown route'
  if (route === 'decision_gateway' || route === 'decide_gateway') return 'Decide Gateway'
  if (route === 'update_gateway_score') return 'Update Gateway'
  if (route === 'routing_evaluate') return 'Rule Evaluate'
  if (route === 'routing_hybrid') return 'Hybrid Routing'
  return humanizeAuditValue(route)
}

export function eventTypeLabel(eventType?: string | null) {
  if (!eventType) return 'Unknown event'
  if (isHybridFlow(eventType)) return 'Hybrid Routing'
  if (eventType === 'decide_gateway_decision') return 'Decide Gateway'
  if (
    eventType === 'update_gateway_score_update' ||
    eventType === 'update_gateway_score_score_snapshot' ||
    eventType === 'update_score_legacy_score_snapshot'
  ) return 'Update Gateway'
  if (isRuleHitFlow(eventType)) return 'Rule Evaluate'
  if (isPreviewFlow(eventType)) return 'Decision Result'
  if (isErrorFlow(eventType)) return 'Errors'
  return humanizeAuditValue(eventType)
}

export function stageLabel(event: PaymentAuditEvent) {
  const flowType = flowTypeValue(event)
  if (event.event_stage === 'hybrid_routed' || isHybridFlow(flowType)) return 'Hybrid Routing'
  if (event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (event.event_stage === 'score_updated' || event.event_stage === 'score_skipped') return 'Update Gateway'
  if (event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (event.event_stage === 'preview_evaluated' || isPreviewFlow(flowType)) return 'Decision Result'
  if (isErrorFlow(flowType)) return 'Errors'
  return humanizeAuditValue(event.event_stage || flowType)
}

/**
 * The group an event is filed under in a timeline. Decision Audit calls the preview group
 * "Rule Decision" (it lists rule evaluations next to live decisions); Explorer and Simulator show
 * previews in their own panel and call it "Decision".
 */
export function eventPhase(event: PaymentAuditEvent, previewPhase = 'Decision') {
  const flowType = flowTypeValue(event)
  if (isHybridFlow(flowType) || event.event_stage === 'hybrid_routed') return 'Hybrid Routing'
  if (isDecisionFlow(flowType) || event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (isRuleHitFlow(flowType) || event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (isUpdateFlow(flowType) || event.event_stage === 'score_updated') return 'Update Gateway'
  if (isPreviewFlow(flowType) || event.event_stage === 'preview_evaluated') return previewPhase
  return 'Errors'
}
