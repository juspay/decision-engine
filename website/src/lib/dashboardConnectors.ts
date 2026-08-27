/**
 * Connectors handed over by the Hyperswitch dashboard when it deep-links into a routing page.
 *
 * The dashboard appends them to the deep-link as a `#connectors=` fragment. A fragment is never
 * sent to a server, so the list stays out of this app's access logs and request-line limits.
 *
 * Read it at module load, which runs while the import graph resolves — before React renders, and
 * before App rewrites the URL to drop the one-time `?code=` (that rewrite drops the fragment with
 * it). Mirror it into sessionStorage so a reload of the same tab keeps the list: sessionStorage is
 * per-tab and dies with the tab, which is exactly the lifetime of one dashboard hand-off.
 */
export interface DashboardConnector {
  merchant_connector_id: string
  connector_name: string
  connector_label: string
}

const STORAGE_KEY = 'hs-dashboard-connectors'
const FRAGMENT_KEY = 'connectors'

/** The dashboard is a separate client, so treat its payload as untrusted and keep only valid rows. */
function parseConnectors(raw: string): DashboardConnector[] | null {
  try {
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return null
    const connectors = parsed.filter(
      (c): c is DashboardConnector =>
        !!c &&
        typeof c === 'object' &&
        typeof (c as DashboardConnector).merchant_connector_id === 'string' &&
        typeof (c as DashboardConnector).connector_name === 'string',
    )
    return connectors.length > 0 ? connectors : null
  } catch {
    return null
  }
}

function readFragment(): DashboardConnector[] | null {
  const hash = window.location.hash.replace(/^#/, '')
  if (!hash) return null
  const raw = new URLSearchParams(hash).get(FRAGMENT_KEY)
  return raw ? parseConnectors(raw) : null
}

function readStorage(): DashboardConnector[] | null {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY)
    return raw ? parseConnectors(raw) : null
  } catch {
    return null
  }
}

function capture(): DashboardConnector[] | null {
  const fromFragment = readFragment()
  if (!fromFragment) return readStorage()
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(fromFragment))
  } catch {
    // Private-mode or a full quota — the in-memory copy below still serves this page load.
  }
  return fromFragment
}

const dashboardConnectors = capture()

/** The profile's connectors when this tab was opened from the dashboard, otherwise `null`. */
export function getDashboardConnectors(): DashboardConnector[] | null {
  return dashboardConnectors
}
