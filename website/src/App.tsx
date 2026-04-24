import { Routes, Route, Navigate } from 'react-router-dom'
import { AppShell } from './components/layout/AppShell'
import { AuthGuard } from './components/layout/AuthGuard'
import { AuthPage } from './pages/AuthPage'
import { OnboardingPage } from './pages/OnboardingPage'
import { OverviewPage } from './components/pages/OverviewPage'
import { RoutingHubPage } from './components/pages/RoutingHubPage'
import { SRRoutingPage } from './components/pages/SRRoutingPage'
import { EuclidRulesPage } from './components/pages/EuclidRulesPage'
import { VolumeSplitPage } from './components/pages/VolumeSplitPage'
import { DebitRoutingPage } from './components/pages/DebitRoutingPage'
import { DecisionExplorerPage } from './components/pages/DecisionExplorerPage'

export default function App() {
  return (
    <Routes>
      <Route path="login" element={<AuthPage />} />
      <Route element={<AuthGuard />}>
        <Route path="onboarding" element={<OnboardingPage />} />
        <Route element={<AppShell />}>
          <Route index element={<OverviewPage />} />
          <Route path="routing" element={<RoutingHubPage />} />
          <Route path="routing/sr" element={<SRRoutingPage />} />
          <Route path="routing/rules" element={<EuclidRulesPage />} />
          <Route path="routing/volume" element={<VolumeSplitPage />} />
          <Route path="routing/debit" element={<DebitRoutingPage />} />
          <Route path="decisions" element={<DecisionExplorerPage />} />
          <Route path="*" element={<Navigate to="." replace />} />
        </Route>
      </Route>
    </Routes>
  )
}
