import { Navigate, Route, Routes } from 'react-router-dom'
import { useEffect } from 'react'
import { AnalyticsPage } from './components/pages/AnalyticsPage'
import { DecisionExplorerPage } from './components/pages/DecisionExplorerPage'
import { DecisionSimulatorPage } from './components/pages/DecisionSimulatorPage'
import { DebitRoutingPage } from './components/pages/DebitRoutingPage'
import { EuclidRulesPage } from './components/pages/EuclidRulesPage'
import { OverviewPage } from './components/pages/OverviewPage'
import { PaymentAuditPage } from './components/pages/PaymentAuditPage'
import { RoutingEventsPage } from './components/pages/RoutingEventsPage'
import { RoutingHubPage } from './components/pages/RoutingHubPage'
import { SRRoutingPage } from './components/pages/SRRoutingPage'
import { VolumeSplitPage } from './components/pages/VolumeSplitPage'
import { ABTestingPage } from './components/pages/ABTestingPage'
import { AppShell } from './components/layout/AppShell'
import { AuthGuard } from './components/layout/AuthGuard'
import { AuthPage } from './pages/AuthPage'
import { OnboardingPage } from './pages/OnboardingPage'
import { MembersPage } from './pages/MembersPage'
import { ApiKeysPage } from './pages/ApiKeysPage'
import { VerifyEmailPage } from './pages/VerifyEmailPage'
import { AccountPage } from './pages/AccountPage'
import { useAuthStore } from './store/authStore'
import { useMerchantStore } from './store/merchantStore'
import { apiPost } from './lib/api'

interface ExchangeResponse {
  token: string
  user_id: string
  email: string
  merchant_id: string
  role: string
}

// Module-scoped guard: the one-time SSO code must be exchanged exactly once, even under
// React StrictMode's double-invoked effects (the code is single-use — a second exchange 401s).
let hsSsoExchangeStarted = false

export default function App() {
  const setAuth = useAuthStore((s) => s.setAuth)
  const setMerchantId = useMerchantStore((s) => s.setMerchantId)

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const code = params.get('code')
    if (!code || hsSsoExchangeStarted) return
    hsSsoExchangeStarted = true

    const stripCode = () => {
      // Strip the code from the URL so it doesn't linger in the address bar or history.
      params.delete('code')
      const newSearch = params.toString()
      const newUrl = window.location.pathname + (newSearch ? `?${newSearch}` : '')
      window.history.replaceState(null, '', newUrl)
    }

    void (async () => {
      try {
        // Redeem the one-time code for a session token. The token is only ever returned in this
        // POST response body — it never appears in the URL.
        const res = await apiPost<ExchangeResponse>(
          '/auth/admin/merchant-token/exchange',
          { code },
        )
        const merchantId = res.merchant_id ?? ''
        setAuth(res.token, {
          userId: res.user_id ?? '',
          email: res.email ?? '',
          merchantId,
          role: res.role ?? 'admin',
          isRedirectSession: true,
        })
        if (merchantId) setMerchantId(merchantId)
      } catch {
        // Invalid / expired / already-used code — leave the user unauthenticated and let
        // AuthGuard redirect to login.
      } finally {
        stripCode()
      }
    })()
  }, [setAuth, setMerchantId])

  return (
    <Routes>
      <Route path="login" element={<AuthPage />} />
      <Route path="signup" element={<AuthPage />} />
      <Route path="verify-email" element={<VerifyEmailPage />} />
      <Route element={<AuthGuard />}>
        <Route path="onboarding" element={<OnboardingPage />} />
        <Route element={<AppShell />}>
          <Route index element={<OverviewPage />} />
          <Route path="routing" element={<RoutingHubPage />} />
          <Route path="routing/sr" element={<SRRoutingPage />} />
          <Route path="routing/rules" element={<EuclidRulesPage />} />
          <Route path="routing/volume" element={<VolumeSplitPage />} />
          <Route path="routing/debit" element={<DebitRoutingPage />} />
          {/* Cost Estimation moved into the Multi Objective page as a tab; keep the
              old path working for bookmarks/links. */}
          <Route path="routing/cost" element={<Navigate to="/routing/sr?tab=cost" replace />} />
          <Route path="routing/ab-testing" element={<ABTestingPage />} />
          <Route path="decisions" element={<DecisionExplorerPage />} />
          <Route path="decisions/simulator" element={<DecisionSimulatorPage />} />
          <Route path="analytics" element={<AnalyticsPage />} />
          <Route path="audit" element={<PaymentAuditPage />} />
          <Route path="events" element={<RoutingEventsPage />} />
          <Route path="members" element={<MembersPage />} />
          <Route path="api-keys" element={<ApiKeysPage />} />
          <Route path="account" element={<AccountPage />} />
          <Route path="*" element={<Navigate to="." replace />} />
        </Route>
      </Route>
    </Routes>
  )
}
