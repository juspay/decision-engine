/**
 * Keeps an open tab from running a dashboard build older than the one deployed.
 *
 * A tab runs the code it loaded with until its document is reloaded, and moving between pages
 * inside the app never reloads it, so a tab left open across a deploy keeps the old behaviour for
 * as long as it stays open. Each build stamps an id into its own code and writes the same id to
 * `version.json` beside it; the tab polls that file and, once it differs, reloads on the next
 * navigation. The page is being replaced at that moment anyway, so nothing on screen is lost.
 *
 * Dev builds carry no id and never check.
 */

const CHECK_INTERVAL_MS = 5 * 60_000

// The deployed build this tab last reloaded for, so a CDN still serving the old page after the
// reload cannot turn every navigation into another reload.
const RELOADED_FOR_KEY = 'de-reloaded-for-build'

let newerBuild: string | null = null

/** The id of the build currently deployed, or `null` when it cannot be read. */
async function fetchDeployedBuild(): Promise<string | null> {
  try {
    const res = await fetch(`${import.meta.env.BASE_URL}version.json?t=${Date.now()}`, {
      cache: 'no-store',
    })
    if (!res.ok) return null
    const body = (await res.json()) as { build?: unknown }
    return typeof body.build === 'string' && body.build ? body.build : null
  } catch {
    // Offline, or the SPA fallback answered with index.html because no version.json is deployed.
    return null
  }
}

/**
 * Check for a newer deployed build now, every few minutes, and whenever the tab comes back into
 * view. Returns the unsubscribe.
 */
export function watchForNewerBuild(): () => void {
  if (!__DASHBOARD_BUILD_ID__) return () => {}

  const check = async () => {
    if (newerBuild) return
    const deployed = await fetchDeployedBuild()
    if (deployed && deployed !== __DASHBOARD_BUILD_ID__) newerBuild = deployed
  }
  const onVisibilityChange = () => {
    if (document.visibilityState === 'visible') void check()
  }

  void check()
  const timer = window.setInterval(() => void check(), CHECK_INTERVAL_MS)
  document.addEventListener('visibilitychange', onVisibilityChange)
  return () => {
    window.clearInterval(timer)
    document.removeEventListener('visibilitychange', onVisibilityChange)
  }
}

/** Reload into the deployed build if it is newer than this tab's, at most once per build. */
export function reloadIfNewerBuild(): void {
  if (!newerBuild) return
  try {
    if (sessionStorage.getItem(RELOADED_FOR_KEY) === newerBuild) return
    sessionStorage.setItem(RELOADED_FOR_KEY, newerBuild)
  } catch {
    // Without a record of the reload there is no guard against repeating it, so stay put.
    return
  }
  window.location.reload()
}
