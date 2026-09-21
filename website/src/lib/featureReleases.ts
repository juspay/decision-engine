import type { AuthUser } from '../store/authStore'
import { useAuthStore } from '../store/authStore'

/**
 * Who a pre-GA feature is currently released to. Presentation only — it hides surfaces, never
 * secures APIs; a route that must refuse non-admins needs its own backend guard.
 */
export type ReleaseAudience = 'everyone' | 'super_admin' | 'nobody'

/**
 * The release roster. To gate a new feature, add it here as 'super_admin' and wrap its surfaces
 * in useFeatureReleased(); to release it, flip the entry to 'everyone', then delete the entry —
 * the compiler then lists every call site left to unwrap. 'nobody' is the emergency retract.
 */
export const FEATURE_RELEASES = {
  'volume-contracts': 'super_admin',
} as const satisfies Record<string, ReleaseAudience>

export type ReleasedFeature = keyof typeof FEATURE_RELEASES

function audienceAdmits(user: AuthUser | null, audience: ReleaseAudience): boolean {
  switch (audience) {
    case 'everyone':
      return true
    case 'nobody':
      return false
    case 'super_admin':
      return Boolean(user?.isSuperAdmin)
  }
}

/**
 * Non-hook evaluator for list filters and plain helpers. Fails closed on a null user — unlike
 * sessionAllows, which fails open: an unreleased surface briefly shown is a leak, while a
 * permission gate guessed wrong is just a control the backend refuses.
 */
export function releaseAdmits(user: AuthUser | null, feature: ReleasedFeature): boolean {
  return audienceAdmits(user, FEATURE_RELEASES[feature])
}

/** Whether this session sees `feature`'s surfaces. */
export function useFeatureReleased(feature: ReleasedFeature): boolean {
  return useAuthStore((s) => releaseAdmits(s.user, feature))
}
