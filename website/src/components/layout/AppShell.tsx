import { useEffect } from 'react'
import { Outlet, useLocation } from 'react-router-dom'
import { RoutingEventsProvider } from '../../hooks/useRoutingEvents'
import { useHeaderAutoHide } from '../../hooks/useHeaderAutoHide'
import { Sidebar } from './Sidebar'
import { TopBar } from './TopBar'
import { SuperAdminBanner } from './SuperAdminBanner'
import { ReadOnlyBanner } from './ReadOnlyBanner'
import { isEmbedded } from '../../lib/embedMode'
import { postToDashboard } from '../../lib/embedBridge'

/**
 * Mirrors the frame's location to the embedding dashboard, which keeps it in its own URL so a
 * dashboard-level refresh or shared link restores the exact page. Rendered inside the router so
 * useLocation sees every navigation; posting is a no-op outside the embed.
 */
function EmbedRouteReporter() {
  const location = useLocation()
  useEffect(() => {
    postToDashboard({ type: 'de:route-changed', path: location.pathname + location.search })
  }, [location])
  return null
}

export function AppShell() {
  // Embedded in the dashboard's routing workspace, the dashboard owns all navigation chrome —
  // this app renders as a bare content pane.
  const embedded = isEmbedded()
  const { scrollRef, headerRef, hidden } = useHeaderAutoHide<HTMLDivElement, HTMLElement>()

  return (
    <RoutingEventsProvider>
      <div
        className={
          // Embedded, render a plain white background — no decorative gradient tint and no
          // colour transition, so the frame is flush with the dashboard's own chrome.
          embedded
            ? 'relative flex h-screen overflow-hidden bg-white text-slate-900'
            : 'relative flex h-screen overflow-hidden bg-[#ffffff] text-slate-900 transition-colors duration-300 dark:bg-[#030507] dark:text-white'
        }
      >
        {!embedded && (
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_left,_rgba(59,130,246,0.05),_transparent_22%),radial-gradient(circle_at_top_right,_rgba(14,165,233,0.04),_transparent_20%),linear-gradient(180deg,_rgba(255,255,255,1),_rgba(255,255,255,1))] dark:bg-[radial-gradient(circle_at_top_left,_rgba(56,189,248,0.06),_transparent_22%),linear-gradient(180deg,_rgba(3,5,7,1),_rgba(5,8,12,1))]" />
        )}
        {!embedded && <div className="aurora-top" />}
        {!embedded && <Sidebar />}
        <div className="flex-1 flex flex-col overflow-hidden relative z-10">
          {/* Banners stay outside the scroller: they describe the session, not the page, so they
              must not scroll away with it. */}
          <SuperAdminBanner />
          <ReadOnlyBanner />
          {/* The scroller owns both the header and the page, which is what lets the header be
              sticky and hand its 78px back to the page on the way down. */}
          <div ref={scrollRef} className="relative flex-1 overflow-y-auto">
            {!embedded && <TopBar hidden={hidden} headerRef={headerRef} />}
            <main className="relative px-4 py-5 sm:px-5 sm:py-6 lg:px-6 lg:py-7 xl:px-8">
              {/* No horizontal padding here — `main` already owns the page gutter, and padding both
                  nested the gutter twice (44px at xl), pushing every page's content off the sidebar. */}
              <EmbedRouteReporter />
              <div className="mx-auto w-full max-w-[1760px]">
                <Outlet />
              </div>
            </main>
          </div>
        </div>
      </div>
    </RoutingEventsProvider>
  )
}
