import { Navigate, Route, Routes } from 'react-router-dom'
import { AnalyticsPage } from './components/pages/AnalyticsPage'
import { DecisionExplorerPage } from './components/pages/DecisionExplorerPage'
import { DecisionSimulatorPage } from './components/pages/DecisionSimulatorPage'
import { DebitRoutingPage } from './components/pages/DebitRoutingPage'
import { EuclidRulesPage } from './components/pages/EuclidRulesPage'
import { OverviewPage } from './components/pages/OverviewPage'
import { PaymentAuditPage } from './components/pages/PaymentAuditPage'
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

export default function App() {
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
          <Route path="routing/ab-testing" element={<ABTestingPage />} />
          <Route path="decisions" element={<DecisionExplorerPage />} />
          <Route path="decisions/simulator" element={<DecisionSimulatorPage />} />
          <Route path="analytics" element={<AnalyticsPage />} />
          <Route path="audit" element={<PaymentAuditPage />} />
          <Route path="members" element={<MembersPage />} />
          <Route path="api-keys" element={<ApiKeysPage />} />
          <Route path="account" element={<AccountPage />} />
          <Route path="*" element={<Navigate to="." replace />} />
        </Route>
      </Route>
    </Routes>
  )
}
