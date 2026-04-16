import { Routes, Route, Navigate } from 'react-router-dom'
import { AppShell } from './components/layout/AppShell'
import { OverviewPage } from './components/pages/OverviewPage'
import { RoutingHubPage } from './components/pages/RoutingHubPage'
import { SRRoutingPage } from './components/pages/SRRoutingPage'
import { EuclidRulesPage } from './components/pages/EuclidRulesPage'
import { VolumeSplitPage } from './components/pages/VolumeSplitPage'
import { DebitRoutingPage } from './components/pages/DebitRoutingPage'
import { DecisionExplorerPage } from './components/pages/DecisionExplorerPage'
import { AnalyticsPage } from './components/pages/AnalyticsPage'

export default function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<OverviewPage />} />
        <Route path="routing" element={<RoutingHubPage />} />
        <Route path="routing/sr" element={<SRRoutingPage />} />
        <Route path="routing/rules" element={<EuclidRulesPage />} />
        <Route path="routing/volume" element={<VolumeSplitPage />} />
        <Route path="routing/debit" element={<DebitRoutingPage />} />
        <Route path="decisions" element={<DecisionExplorerPage />} />
        <Route path="analytics" element={<AnalyticsPage />} />
        <Route path="*" element={<Navigate to="." replace />} />
      </Route>
    </Routes>
  )
}
