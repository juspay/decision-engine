import { useState } from 'react'
import { Play, ChevronDown } from 'lucide-react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Spinner } from '../ui/Spinner'
import { apiPost } from '../../lib/api'
import { DecideGatewayResponse, RoutingAlgorithmName } from '../../types/api'
import { GsmOptionRow } from './ErrorInfoFields'

export interface GsmScenario {
  key: string
  label: string
  penalise: boolean
  errorCategory: string
  decision?: string
}

interface ScenarioTestResult {
  scoreBefore: number | null
  scoreAfter: number | null
  loading: boolean
  error: string | null
}

export interface DecideParams {
  amount: string
  currency: string
  paymentMethodType: string
  paymentMethod: string
  authType: string
  cardBrand: string
  rankingAlgorithm: RoutingAlgorithmName
  eligibleGateways: string
}

interface Props {
  merchantId: string | null
  gsmScenarios: GsmScenario[]
  gsmRules: GsmOptionRow[]
  decideParams: DecideParams
}

export function PenaltyClassificationGuide({ merchantId, gsmScenarios, gsmRules, decideParams }: Props) {
  const [expandedScenario, setExpandedScenario] = useState<string | null>(null)
  const [scenarioConnector, setScenarioConnector] = useState<Record<string, string>>({})
  const [scenarioTestResults, setScenarioTestResults] = useState<Record<string, ScenarioTestResult>>({})

  async function runScenarioTest(scenarioKey: string, connector: string, errorCode: string, errorMessage: string, flow: string, subFlow: string) {
    if (!merchantId) return
    setScenarioTestResults(r => ({ ...r, [scenarioKey]: { scoreBefore: null, scoreAfter: null, loading: true, error: null } }))
    try {
      // SR scoring only runs when ≥2 gateways are eligible (single-gateway path
      // short-circuits to a default 1.0 score, bypassing Redis reads entirely).
      // Always include at least one peer gateway so the SRV3 bucket score is read.
      const formGateways = decideParams.eligibleGateways.split(',').map(g => g.trim()).filter(Boolean)
      const peers = formGateways.filter(g => g !== connector)
      const peerGateway = peers[0] ?? (connector === 'stripe' ? 'adyen' : 'stripe')
      const eligibleGatewayList = [connector, peerGateway]

      const basePayload = {
        merchantId,
        paymentInfo: {
          paymentId: '',
          amount: parseFloat(decideParams.amount) || 1000,
          currency: decideParams.currency || 'USD',
          paymentType: 'ORDER_PAYMENT',
          paymentMethodType: decideParams.paymentMethodType || 'CARD',
          paymentMethod: decideParams.paymentMethod || 'CREDIT',
          authType: decideParams.authType || 'NO_THREE_DS',
          cardBrand: decideParams.cardBrand || 'VISA',
        },
        eligibleGatewayList,
        rankingAlgorithm: decideParams.rankingAlgorithm,
        eliminationEnabled: false,
      }

      // Read score before any injections.
      const seed = Date.now()
      const beforeId = `scenario_test_${seed}_0`
      const before = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        ...basePayload, paymentInfo: { ...basePayload.paymentInfo, paymentId: beforeId },
      })
      const scoreBefore = (before.gateway_priority_map as Record<string, number> | null)?.[connector] ?? null

      // Inject 5 failures with distinct payment IDs so each gets its own
      // GatewayScoringData entry and the moving-window queue is updated 5 times.
      // A single injection may not shift the score visibly if the oldest window
      // entry was also a failure (net change = 0 that round).
      for (let i = 1; i <= 5; i++) {
        const pid = `scenario_test_${seed}_${i}`
        await apiPost<DecideGatewayResponse>('/decide-gateway', {
          ...basePayload, paymentInfo: { ...basePayload.paymentInfo, paymentId: pid },
        })
        await apiPost('/update-gateway-score', {
          merchantId,
          gateway: connector,
          gatewayReferenceId: null,
          status: 'FAILURE',
          paymentId: pid,
          errorInfo: { connector, flow, subFlow, errorCode, errorMessage },
        })
      }

      // Read score after all injections.
      const afterId = `scenario_test_${seed}_after`
      const after = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        ...basePayload, paymentInfo: { ...basePayload.paymentInfo, paymentId: afterId },
      })
      const scoreAfter = (after.gateway_priority_map as Record<string, number> | null)?.[connector] ?? null

      setScenarioTestResults(r => ({ ...r, [scenarioKey]: { scoreBefore, scoreAfter, loading: false, error: null } }))
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Test failed'
      setScenarioTestResults(r => ({ ...r, [scenarioKey]: { scoreBefore: null, scoreAfter: null, loading: false, error: msg } }))
    }
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <h3 className="text-sm font-medium text-slate-800 dark:text-white">Penalty Classification Guide</h3>
          <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
            How the routing engine decides whether a failed payment penalises the gateway score.
          </p>
        </div>
      </CardHeader>
      <CardBody className="space-y-4">
        {gsmScenarios.length === 0 ? (
          <p className="text-xs text-slate-400">Loading scenario config…</p>
        ) : (
          <>
            {(['protected', 'penalized'] as const).map(kind => {
              const group = gsmScenarios.filter(s => (s.penalise ? 'penalized' : 'protected') === kind)
              if (!group.length) return null
              const isProtected = kind === 'protected'
              return (
                <div key={kind}>
                  <p className={`text-[11px] font-semibold uppercase tracking-wider mb-2 ${isProtected ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-500 dark:text-red-400'}`}>
                    {isProtected ? '✓ Protected — gateway score unchanged' : '✗ Penalised — gateway score decremented'}
                  </p>
                  <div className="space-y-2">
                    {group.map(scenario => {
                      const matchingRules = gsmRules.filter(r =>
                        r.errorCategory === scenario.errorCategory &&
                        (!scenario.decision || r.decision === scenario.decision) &&
                        (r.subFlow === 'Authorize' || r.flow === 'Authorize')
                      )
                      const connectors = [...new Set(matchingRules.map(r => r.connector))]
                      const isExpanded = expandedScenario === scenario.key
                      const selectedConn = scenarioConnector[scenario.key] ?? connectors[0] ?? ''
                      const selectedRule = matchingRules.find(r => r.connector === selectedConn)
                      const testResult = scenarioTestResults[scenario.key]
                      const scoreDelta = testResult?.scoreBefore != null && testResult?.scoreAfter != null
                        ? testResult.scoreAfter - testResult.scoreBefore
                        : null
                      const scoreChanged = scoreDelta != null && Math.abs(scoreDelta) > 0.001
                      // For protected scenarios only a decrease is a failure — an increase is fine
                      // (hedging can naturally push the score up during the test window).
                      const isUnexpected = isProtected
                        ? scoreDelta != null && scoreDelta < -0.001
                        : scoreChanged

                      return (
                        <div
                          key={scenario.key}
                          className={`rounded-lg border overflow-hidden transition-all ${isProtected ? 'border-emerald-200 dark:border-emerald-800/40' : 'border-red-200 dark:border-red-800/40'}`}
                        >
                          <button
                            type="button"
                            onClick={() => setExpandedScenario(isExpanded ? null : scenario.key)}
                            className={`w-full flex items-center justify-between px-3 py-2.5 text-left transition-colors ${isProtected ? 'bg-emerald-50/50 dark:bg-emerald-900/10 hover:bg-emerald-50 dark:hover:bg-emerald-900/20' : 'bg-red-50/50 dark:bg-red-900/10 hover:bg-red-50 dark:hover:bg-red-900/20'}`}
                          >
                            <div className="flex items-center gap-2 min-w-0">
                              <span className="text-xs font-semibold text-slate-800 dark:text-slate-100">{scenario.label}</span>
                              <span className="text-[10px] font-mono text-slate-400 dark:text-slate-500 truncate hidden sm:block">
                                {scenario.errorCategory}{scenario.decision ? ` · ${scenario.decision}` : ''}
                              </span>
                            </div>
                            <div className="flex items-center gap-2 shrink-0 ml-2">
                              <span className="text-[10px] text-slate-400">{connectors.length} connector{connectors.length !== 1 ? 's' : ''}</span>
                              <ChevronDown size={12} className={`text-slate-400 transition-transform ${isExpanded ? 'rotate-180' : ''}`} />
                            </div>
                          </button>

                          {isExpanded && (
                            <div className="px-3 py-3 space-y-3 border-t border-slate-100 dark:border-[#1c1c24] bg-white dark:bg-[#0d1117]">
                              <div className="text-[11px] text-slate-500 dark:text-slate-400 space-y-0.5">
                                <span className="font-medium text-slate-700 dark:text-slate-300">Rule: </span>
                                <span className="font-mono">{scenario.errorCategory}</span>
                                {scenario.decision
                                  ? <span> with decision <span className="font-mono">{scenario.decision}</span></span>
                                  : <span> (any decision)</span>
                                }
                              </div>

                              {connectors.length > 0 && (
                                <div>
                                  <p className="text-[10px] font-medium text-slate-500 dark:text-slate-400 mb-1.5">Select connector to test:</p>
                                  <div className="flex flex-wrap gap-1.5">
                                    {connectors.map(conn => (
                                      <button
                                        key={conn}
                                        type="button"
                                        onClick={() => setScenarioConnector(s => ({ ...s, [scenario.key]: conn }))}
                                        className={`rounded-full px-2.5 py-1 text-[11px] font-medium ring-1 ring-inset transition-colors ${
                                          selectedConn === conn
                                            ? isProtected
                                              ? 'bg-emerald-50 text-emerald-700 ring-emerald-300 dark:bg-emerald-900/30 dark:text-emerald-300 dark:ring-emerald-700'
                                              : 'bg-red-50 text-red-700 ring-red-300 dark:bg-red-900/30 dark:text-red-300 dark:ring-red-700'
                                            : 'bg-slate-50 text-slate-600 ring-slate-200 hover:bg-slate-100 dark:bg-[#1c1c24] dark:text-slate-400 dark:ring-[#2a2a35]'
                                        }`}
                                      >
                                        {conn}
                                      </button>
                                    ))}
                                  </div>
                                </div>
                              )}

                              {selectedRule && (
                                <div className="rounded-lg bg-slate-50 dark:bg-[#10131c] px-3 py-2 text-[11px] space-y-0.5">
                                  <div><span className="text-slate-500">Error code: </span><span className="font-mono font-medium text-slate-700 dark:text-slate-200">{selectedRule.errorCode}</span></div>
                                  <div><span className="text-slate-500">Message: </span><span className="text-slate-600 dark:text-slate-300">{selectedRule.errorMessage}</span></div>
                                </div>
                              )}

                              {selectedConn && selectedRule && (
                                <button
                                  type="button"
                                  disabled={!merchantId || testResult?.loading}
                                  onClick={() => runScenarioTest(scenario.key, selectedConn, selectedRule.errorCode, selectedRule.errorMessage, selectedRule.flow, selectedRule.subFlow)}
                                  className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                                    isProtected
                                      ? 'bg-emerald-600 text-white hover:bg-emerald-700'
                                      : 'bg-red-500 text-white hover:bg-red-600'
                                  }`}
                                >
                                  {testResult?.loading ? <><Spinner size={12} /> Injecting 5 failures…</> : <><Play size={12} /> Test with {selectedConn}</>}
                                </button>
                              )}

                              {!merchantId && (
                                <p className="text-[11px] text-amber-600 dark:text-amber-400">Set a merchant ID in the top bar to run tests.</p>
                              )}

                              {testResult && !testResult.loading && (
                                <div className={`rounded-lg border px-3 py-2.5 space-y-1.5 ${
                                  testResult.error
                                    ? 'border-amber-200 bg-amber-50 dark:border-amber-800/40 dark:bg-amber-900/10'
                                    : isUnexpected
                                      ? 'border-amber-200 bg-amber-50 dark:border-amber-800/40 dark:bg-amber-900/10'
                                      : isProtected
                                        ? 'border-emerald-200 bg-emerald-50 dark:border-emerald-800/40 dark:bg-emerald-900/10'
                                        : 'border-red-200 bg-red-50 dark:border-red-800/40 dark:bg-red-900/10'
                                }`}>
                                  {testResult.error ? (
                                    <p className="text-[11px] text-amber-700 dark:text-amber-300">{testResult.error}</p>
                                  ) : (
                                    <>
                                      <div className="flex items-center gap-3 text-[11px]">
                                        <span className="text-slate-500">Score:</span>
                                        <span className="font-mono font-semibold text-slate-700 dark:text-slate-200">
                                          {testResult.scoreBefore?.toFixed(3) ?? '—'}
                                        </span>
                                        <span className="text-slate-400">→</span>
                                        <span className={`font-mono font-semibold ${scoreChanged ? 'text-red-600 dark:text-red-400' : 'text-emerald-600 dark:text-emerald-400'}`}>
                                          {testResult.scoreAfter?.toFixed(3) ?? '—'}
                                        </span>
                                        {scoreDelta != null && (
                                          <span className={`text-[10px] font-medium ${scoreChanged ? 'text-red-500' : 'text-emerald-500'}`}>
                                            ({scoreDelta > 0 ? '+' : ''}{scoreDelta.toFixed(3)})
                                          </span>
                                        )}
                                      </div>
                                      <p className={`text-[11px] font-medium ${
                                        isUnexpected
                                          ? 'text-amber-700 dark:text-amber-300'
                                          : isProtected
                                            ? 'text-emerald-700 dark:text-emerald-300'
                                            : 'text-red-600 dark:text-red-400'
                                      }`}>
                                        {isProtected
                                          ? isUnexpected
                                            ? `⚠ Score decreased — gateway was penalised (${scoreDelta!.toFixed(3)})`
                                            : scoreDelta != null && scoreDelta > 0.001
                                              ? `✓ Score increased (+${scoreDelta.toFixed(3)}) — no penalty applied (hedging)`
                                              : '✓ Score unchanged — gateway not penalised as expected'
                                          : scoreChanged
                                            ? `✗ Gateway penalised (score dropped ${Math.abs(scoreDelta!).toFixed(3)})`
                                            : '⚠ Score unchanged — penalty may not have applied yet'
                                        }
                                      </p>
                                    </>
                                  )}
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      )
                    })}
                  </div>
                </div>
              )
            })}
            <p className="text-[11px] text-slate-400 dark:text-slate-500 pt-1 border-t border-slate-100 dark:border-[#1c1c24]">
              Errors with no matching GSM rule are always penalised. Configure simulation parameters on the left and run to observe score changes live.
            </p>
          </>
        )}
      </CardBody>
    </Card>
  )
}
