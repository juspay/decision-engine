// TEMP probe (review verification) — faithful repro of AuthPage's post-login navigation using the
// app's REAL authStore + REAL dashboardHandoff module. Deleted after the run.
import React, { useEffect, useState } from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter, Route, Routes, useNavigate, useLocation } from 'react-router-dom'
import { useAuthStore } from './store/authStore'
import { takeDashboardRoute, getDashboardConnectors } from './lib/dashboardHandoff'

const log = (...a: unknown[]) => console.log('[P2]', ...a)
log('module load: takeable route present?', JSON.stringify(sessionStorage.getItem('hs-dashboard-handoff')))
log('connectors resurrected:', JSON.stringify(getDashboardConnectors()))

let effectFired = 0

function LoginProbe() {
  const navigate = useNavigate()
  const { token, hasHydrated, setAuth } = useAuthStore()
  const [loading, setLoading] = useState(false)

  // ---- copy of AuthPage.tsx lines 109-114 ----
  useEffect(() => {
    if (!hasHydrated || !token || loading) return
    effectFired += 1
    const r = takeDashboardRoute()
    log('*** AuthPage effect FIRED, takeDashboardRoute() =', r)
    navigate(r ?? '/', { replace: true })
  }, [hasHydrated, loading, navigate, token])

  // ---- copy of AuthPage.tsx handleSubmit success path (lines 164-223) ----
  async function handleSubmit() {
    log('submit start')
    setLoading(true)
    try {
      await new Promise((r) => setTimeout(r, 30)) // stand-in for await apiFetch('/auth/login')
      setAuth('token-abc', { userId: 'u', email: 'e', merchantId: 'm', role: 'admin', isRedirectSession: false }, [{ merchant_id: 'm', merchant_name: 'M', role: 'admin' }])
      localStorage.removeItem('pending_merchant_name')
      navigate('/', { replace: true })
    } finally {
      setLoading(false)
    }
  }

  return <button id="login" onClick={handleSubmit}>login</button>
}

function Where() {
  const loc = useLocation()
  return <div id="where">{loc.pathname}</div>
}

function App() {
  return (
    <>
      <Where />
      <Routes>
        <Route path="/login" element={<LoginProbe />} />
        <Route path="/" element={<div id="home">overview</div>} />
        <Route path="/routing/rules/:id/edit" element={<div id="stale">STALE DEEP LINK PAGE</div>} />
        <Route path="*" element={<div>other</div>} />
      </Routes>
    </>
  )
}

const control = window.location.hash === '#b'
if (control) {
  // Control: AuthPage mounted while a session token already exists (e.g. the user typed /login,
  // or came back to a /login history entry after signing in).
  useAuthStore.getState().setAuth('pre-existing', { userId: 'u', email: 'e', merchantId: 'm', role: 'admin', isRedirectSession: false })
} else {
  useAuthStore.getState().clearAuth()
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
)

setTimeout(() => {
  if (!control) (document.getElementById('login') as HTMLButtonElement | null)?.click()
  setTimeout(() => {
    log('RESULT effectFired=', effectFired, 'url=', window.location.pathname, 'body=', document.getElementById('root')!.textContent)
    log('storage after=', sessionStorage.getItem('hs-dashboard-handoff'))
    ;(window as unknown as Record<string, unknown>).__probeDone = {
      effectFired,
      url: window.location.pathname,
      body: document.getElementById('root')!.textContent,
      storage: sessionStorage.getItem('hs-dashboard-handoff'),
    }
  }, 1500)
}, 800)
