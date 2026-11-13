/**
 * Messages the embedded app posts to the dashboard that frames it.
 *
 * The dashboard reacts by re-minting the hand-off (session-expired) or mirroring the frame's
 * location into its own URL (route-changed) so refresh and share restore the same page.
 *
 * Addressed to exactly `window.location.origin`: in every deployed environment both apps share an
 * origin, so a foreign embedder never receives these. In cross-origin dev the messages drop
 * silently, which degrades to the app handling expiry itself.
 */
import { isEmbedded } from './embedMode'

export type EmbedMessage =
  | { type: 'de:ready' }
  | { type: 'de:route-changed'; path: string }
  | { type: 'de:session-expired' }

export function postToDashboard(message: EmbedMessage): void {
  if (!isEmbedded() || window.parent === window) return
  window.parent.postMessage(message, window.location.origin)
}
