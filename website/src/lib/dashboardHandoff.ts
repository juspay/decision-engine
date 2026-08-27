/**
 * What the Hyperswitch dashboard hands over when it deep-links into a routing page.
 *
 * The dashboard appends it to the deep link as a `#connectors=…&rule_id=…` fragment. A fragment is
 * never sent to a server, so this stays out of this app's access logs and request-line limits.
 *
 * Read it at module load, which runs while the import graph resolves — before React renders, and
 * so before anything can lose it. The intended route in particular is fragile: on a cold load the
 * SSO exchange has not finished when <Routes> first mounts, AuthGuard bounces to /login with a
 * history *replace*, and the deep-linked path is gone before any component could have read it.
 *
 * Mirror into sessionStorage so a reload of the same tab keeps the hand-off: sessionStorage is
 * per-tab and dies with the tab, which is exactly the lifetime of one hand-off.
 */
export interface DashboardConnector {
  merchant_connector_id: string
  connector_name: string
  connector_label: string
}

const STORAGE_KEY = 'hs-dashboard-handoff'
const CONNECTORS_KEY = 'connectors'
const RULE_ID_KEY = 'rule_id'

/**
 * The only DE pages that address a single rule. Hyperswitch's other deep-link targets are
 * profile-level config pages (multi-objective, debit) with no per-rule route, so a rule id means
 * nothing there and must not be appended to the path.
 */
const RULE_ROUTES = ['/routing/rules', '/routing/volume']

interface Handoff {
  connectors: DashboardConnector[] | null
  ruleId: string | null
  /** Where the dashboard meant to land, already resolved to a router path. Consumed once. */
  route: string | null
}

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

function readFragment(): Pick<Handoff, 'connectors' | 'ruleId'> | null {
  const hash = window.location.hash.replace(/^#/, '')
  if (!hash) return null
  const params = new URLSearchParams(hash)
  const rawConnectors = params.get(CONNECTORS_KEY)
  const rawRuleId = params.get(RULE_ID_KEY)
  if (!rawConnectors && !rawRuleId) return null
  return {
    connectors: rawConnectors ? parseConnectors(rawConnectors) : null,
    ruleId: rawRuleId || null,
  }
}

/** window.location.pathname includes the deployment base; the router does not want it. */
function routerPath(): string {
  const base = import.meta.env.BASE_URL.endsWith('/')
    ? import.meta.env.BASE_URL.slice(0, -1)
    : import.meta.env.BASE_URL
  const path = window.location.pathname
  return base && path.startsWith(base) ? path.slice(base.length) || '/' : path
}

/**
 * A same-origin absolute path, and nothing else.
 *
 * The fragment and the URL are attacker-supplied, so a crafted link like `http://host//evil.com`
 * arrives as pathname `//evil.com`. That resolves protocol-relative, and `history.pushState`
 * rejects it with a SecurityError — which would throw straight out of the navigate below and leave
 * the app wedged on its "signing you in" loader. Leading `\` is the same trick in some browsers.
 */
function isSameOriginPath(path: string): boolean {
  return /^\/(?![/\\])/.test(path)
}

/**
 * Only a hand-off has an intended route worth restoring — a plain page load routes itself, and
 * force-navigating there would fight normal use. `?code=` is what marks the arrival.
 */
function intendedRoute(ruleId: string | null): string | null {
  if (!new URLSearchParams(window.location.search).has('code')) return null
  const path = routerPath().replace(/\/+$/, '') || '/'
  if (!isSameOriginPath(path)) return null
  // The id is a path segment, so it must be escaped — an id of "../../admin" would otherwise
  // climb out of the route it belongs to.
  return ruleId && RULE_ROUTES.includes(path)
    ? `${path}/${encodeURIComponent(ruleId)}/edit`
    : path
}

function persist(value: Handoff): void {
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(value))
  } catch {
    // Private mode or a full quota — the in-memory copy still serves this page load.
  }
}

function readStorage(): Handoff | null {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    const parsed: unknown = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object') return null
    const { connectors, ruleId, route } = parsed as Record<string, unknown>
    return {
      connectors: Array.isArray(connectors) ? parseConnectors(JSON.stringify(connectors)) : null,
      ruleId: typeof ruleId === 'string' && ruleId ? ruleId : null,
      route: typeof route === 'string' && route ? route : null,
    }
  } catch {
    return null
  }
}

function capture(): Handoff | null {
  const stored = readStorage()
  const fragment = readFragment()
  const connectors = fragment?.connectors ?? stored?.connectors ?? null
  const ruleId = fragment?.ruleId ?? stored?.ruleId ?? null
  const route = intendedRoute(ruleId) ?? stored?.route ?? null
  if (!connectors && !ruleId && !route) return null
  const next: Handoff = { connectors, ruleId, route }
  persist(next)
  return next
}

const handoff = capture()

/** The profile's connectors when this tab was opened from the dashboard, otherwise `null`. */
export function getDashboardConnectors(): DashboardConnector[] | null {
  return handoff?.connectors ?? null
}

/** The rule the merchant clicked in Hyperswitch, when this tab was opened from the dashboard. */
export function getDashboardRuleId(): string | null {
  return handoff?.ruleId ?? null
}

/**
 * Where the dashboard meant to land. Consumed on read so it steers the one navigation after the
 * session is established and never hijacks the user's own navigation afterwards.
 */
export function takeDashboardRoute(): string | null {
  if (!handoff?.route) return null
  const { route } = handoff
  handoff.route = null
  persist(handoff)
  return route
}
