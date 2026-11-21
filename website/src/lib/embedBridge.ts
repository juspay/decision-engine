/**
 * Messages the embedded app posts to the dashboard that frames it.
 */
import { isEmbedded } from './embedMode'
import { applyThemeTokens } from './theme'

export type EmbedMessage =
  | { type: 'de:ready' }
  | { type: 'de:route-changed'; path: string }
  | { type: 'de:session-expired' }
  | { type: 'de:theme-ready'; version: number; frameId: string }
  | { type: 'de:theme-applied'; version: number; frameId: string; revision: number }

export function postToDashboard(message: EmbedMessage): void {
  if (!isEmbedded() || window.parent === window) return
  window.parent.postMessage(message, window.location.origin)
}

let frameId = ''

export function setupEmbedBridge(): void {
  if (!isEmbedded() || window.parent === window) return

  // Generate a random frame ID for this session
  frameId = Math.random().toString(36).substring(2, 9)

  window.addEventListener('message', (event) => {
    // Exact origin match and only from parent
    if (event.origin !== window.location.origin || event.source !== window.parent) return

    const { data } = event
    if (data && data.type === 'de:theme-update' && data.version === 1) {
      if (data.frameId !== frameId) return // stale or wrong frame
      
      // Apply theme
      if (data.tokens && typeof data.tokens === 'object') {
        applyThemeTokens(data.tokens)
      }

      // Ack
      postToDashboard({ type: 'de:theme-applied', version: 1, frameId, revision: data.revision })
    }
  })

  // Signal ready to receive theme
  postToDashboard({ type: 'de:theme-ready', version: 1, frameId })
}
