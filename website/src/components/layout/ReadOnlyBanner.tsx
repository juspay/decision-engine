import { Eye } from 'lucide-react'
import { useAuthStore, sessionAllows } from '../../store/authStore'

// Shown when the session may look at routing but not change it. Without it the dashboard would
// appear fully editable right up to the moment a save is refused, which reads as a broken page
// rather than as the permissions the user was actually given.
export function ReadOnlyBanner() {
  const user = useAuthStore((s) => s.user)
  const canEdit = useAuthStore((s) => sessionAllows(s.user, 'routing:write'))

  if (canEdit) return null

  return (
    <div className="flex h-9 shrink-0 items-center justify-center gap-2 bg-slate-700 px-4 text-[12.5px] font-medium text-slate-100 dark:bg-[#1e2430]">
      <Eye size={14} className="shrink-0" />
      <span>
        View only — your Hyperswitch role can see routing for{' '}
        <span className="font-semibold">{user?.merchantId}</span> but not change it.
      </span>
    </div>
  )
}
