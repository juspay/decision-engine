/**
 * Whether this app is running chrome-less inside the Hyperswitch dashboard's iframe.
 *
 * The dashboard's routing workspace appends `?embed=1` to the hand-off URL. Read it at module
 * load: the SSO code strip and every router navigation rewrite the URL, so the query param is
 * gone moments after arrival. Mirror into sessionStorage so an in-frame reload stays chrome-less.
 *
 * Embed mode also requires actually being framed. sessionStorage is shared per-tab across
 * same-origin documents — the flag written inside the dashboard's iframe is visible to a later
 * top-level load of this app in the same tab, and window.open() clones the opener's
 * sessionStorage into the new tab ("Open in new tab" would inherit it). A chrome-less top-level
 * page whose expiry handling posts to a parent that isn't there is a dead end, so a top-level
 * load never honors the flag and scrubs it to keep the tab's next load clean.
 */
const STORAGE_KEY = 'de-embed-mode'

function capture(): boolean {
  if (window.self === window.top) {
    try {
      sessionStorage.removeItem(STORAGE_KEY)
    } catch {
      // Private mode — nothing stored, nothing to scrub.
    }
    return false
  }
  if (new URLSearchParams(window.location.search).get('embed') === '1') {
    try {
      sessionStorage.setItem(STORAGE_KEY, '1')
    } catch {
      // Private mode or a full quota — the in-memory copy still serves this page load.
    }
    return true
  }
  try {
    return sessionStorage.getItem(STORAGE_KEY) === '1'
  } catch {
    return false
  }
}

const embedded = capture()

/** True when this page load belongs to the dashboard's embedded routing workspace. */
export function isEmbedded(): boolean {
  return embedded
}

/**
 * Inset classes for a full-bleed overlay. Standalone, an overlay has to clear this app's own
 * chrome (16rem sidebar, 76px top bar); embedded, the dashboard renders that chrome outside the
 * iframe, so reserving the same space leaves the overlay short of the frame on two sides.
 * Pass false for overlays that already span the full width.
 */
export function overlayInsetClass(clearsSidebar = true): string {
  if (embedded) return 'inset-0'
  return clearsSidebar ? 'bottom-0 left-64 right-0 top-[76px]' : 'bottom-0 left-0 right-0 top-[76px]'
}
