import { Navigate, Route, Routes } from 'react-router-dom'
import { useEffect, useState } from 'react'
import { AnalyticsPage } from './components/pages/AnalyticsPage'
import { DecisionExplorerPage } from './components/pages/DecisionExplorerPage'
import { DecisionSimulatorPage } from './components/pages/DecisionSimulatorPage'
import { DebitRoutingPage } from './components/pages/DebitRoutingPage'
import { EuclidRulesPage } from './components/pages/EuclidRulesPage'
import { VolumeContractsPage } from './components/pages/VolumeContractsPage'
import { EuclidRuleBuilderPage } from './components/pages/EuclidRuleBuilderPage'
import { VolumeSplitBuilderPage } from './components/pages/VolumeSplitBuilderPage'
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
import { ForgotPasswordPage } from './pages/ForgotPasswordPage'
import { ResetPasswordPage } from './pages/ResetPasswordPage'
import { AccountPage } from './pages/AccountPage'
import { signupEnabled, simulatorEnabled } from './lib/appConfig'
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
  // Hold the app on a loader while an SSO `?code=` is being exchanged. Without this, <Routes>
  // (and AuthGuard) render immediately; AuthGuard's child effect navigates to /login and strips
  // the code from the URL before this parent effect can read it, so the exchange never fires.
  const [exchangingCode, setExchangingCode] = useState(
    () => new URLSearchParams(window.location.search).has('code'),
  )

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const code = params.get('code')
    if (!code || hsSsoExchangeStarted) {
      setExchangingCode(false)
      return
    }
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
        setExchangingCode(false)
      }
    })()
  }, [setAuth, setMerchantId])

  if (exchangingCode) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-white text-sm text-slate-600 dark:bg-[#030507] dark:text-[#c7cfdb]">
        Signing you in…
      </div>
    )
  }

  return (
    <Routes>
      <Route path="login" element={<AuthPage />} />
      {signupEnabled ? <Route path="signup" element={<AuthPage />} /> : null}
      <Route path="verify-email" element={<VerifyEmailPage />} />
      <Route path="forgot-password" element={<ForgotPasswordPage />} />
      <Route path="reset-password" element={<ResetPasswordPage />} />
      <Route element={<AuthGuard />}>
        <Route path="onboarding" element={<OnboardingPage />} />
        <Route element={<AppShell />}>
          <Route index element={<OverviewPage />} />
          <Route path="routing" element={<RoutingHubPage />} />
          <Route path="routing/sr" element={<SRRoutingPage />} />
          <Route path="routing/rules" element={<EuclidRulesPage />} />
          <Route path="routing/rules/new" element={<EuclidRuleBuilderPage />} />
          <Route path="routing/rules/:id/edit" element={<EuclidRuleBuilderPage />} />
          <Route path="routing/volume" element={<VolumeSplitPage />} />
          <Route path="routing/volume/new" element={<VolumeSplitBuilderPage />} />
          <Route path="routing/volume/:id/edit" element={<VolumeSplitBuilderPage />} />
          <Route path="routing/debit" element={<DebitRoutingPage />} />
          <Route path="routing/volume-contracts" element={<VolumeContractsPage />} />
          {/* Cost Estimation moved into the Multi Objective page as a tab; keep the
              old path working for bookmarks/links. */}
          <Route path="routing/cost" element={<Navigate to="/routing/sr?tab=cost" replace />} />
          <Route path="routing/ab-testing" element={<ABTestingPage />} />
          <Route path="decisions" element={<DecisionExplorerPage />} />
          {simulatorEnabled ? (
            <Route path="decisions/simulator" element={<DecisionSimulatorPage />} />
          ) : null}
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
