import { useState, useEffect } from 'react'
import { UserPlus, Users, Copy, Check, Loader2, Eye, EyeOff } from 'lucide-react'
import { apiFetch } from '../lib/api'
import { ErrorMessage } from '../components/ui/ErrorMessage'

interface MemberInfo {
  user_id: string
  email: string
  role: string
}

interface InviteResponse {
  email: string
  is_new_user: boolean
  password?: string
  role: string
}

function RoleBadge({ role }: { role: string }) {
  const colors =
    role === 'admin'
      ? 'bg-violet-50 text-violet-700 dark:bg-violet-950/40 dark:text-violet-300'
      : 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300'
  return (
    <span className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-[11px] font-semibold capitalize ${colors}`}>
      {role}
    </span>
  )
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  function copy() {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1800)
    })
  }
  return (
    <button
      onClick={copy}
      className="ml-1.5 inline-flex items-center justify-center w-6 h-6 rounded text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 transition-colors"
      title="Copy"
    >
      {copied ? <Check size={13} className="text-green-500" /> : <Copy size={13} />}
    </button>
  )
}

export function MembersPage() {
  const [members, setMembers] = useState<MemberInfo[]>([])
  const [loadingMembers, setLoadingMembers] = useState(true)
  const [membersError, setMembersError] = useState<string | null>(null)

  const [email, setEmail] = useState('')
  const [role, setRole] = useState('member')
  const [inviting, setInviting] = useState(false)
  const [inviteError, setInviteError] = useState<string | null>(null)
  const [inviteResult, setInviteResult] = useState<InviteResponse | null>(null)
  const [showPassword, setShowPassword] = useState(false)

  async function loadMembers() {
    setLoadingMembers(true)
    setMembersError(null)
    try {
      const data = await apiFetch<MemberInfo[]>('/merchant/members')
      setMembers(data)
    } catch (err) {
      setMembersError(err instanceof Error ? err.message : 'Failed to load members')
    } finally {
      setLoadingMembers(false)
    }
  }

  useEffect(() => {
    loadMembers()
  }, [])

  async function handleInvite(e: React.FormEvent) {
    e.preventDefault()
    setInviteError(null)
    setInviteResult(null)
    setInviting(true)
    try {
      const res = await apiFetch<InviteResponse>('/merchant/members/invite', {
        method: 'POST',
        body: JSON.stringify({ email, role }),
      })
      setInviteResult(res)
      setEmail('')
      loadMembers()
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Something went wrong'
      const match = msg.match(/API error \d+: (.+)/)
      if (match) {
        try {
          const parsed = JSON.parse(match[1])
          setInviteError(parsed.message ?? msg)
        } catch {
          setInviteError(match[1])
        }
      } else {
        setInviteError(msg)
      }
    } finally {
      setInviting(false)
    }
  }

  return (
    <div className="mx-auto max-w-3xl space-y-8 px-4 py-8">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-slate-900 dark:text-white">
          Members
        </h1>
        <p className="mt-1 text-sm text-slate-500 dark:text-[#8a94a7]">
          Manage who has access to this merchant workspace.
        </p>
      </div>

      {/* Invite form */}
      <div className="rounded-2xl border border-slate-200 bg-white p-6 dark:border-[#1d2029] dark:bg-[#0c0e14]">
        <div className="flex items-center gap-2.5 mb-5">
          <UserPlus size={18} className="text-brand-600 dark:text-sky-400" />
          <h2 className="text-base font-semibold text-slate-900 dark:text-white">Invite member</h2>
        </div>

        <form onSubmit={handleInvite} className="space-y-4">
          <div className="flex gap-3">
            <input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="colleague@example.com"
              className="h-10 flex-1 rounded-xl border border-slate-200 bg-white px-3.5 text-sm text-slate-900 outline-none transition placeholder:text-slate-400 focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 dark:border-[#2a2d35] dark:bg-[#1a1d25] dark:text-white dark:placeholder:text-[#6e7684] dark:focus:border-blue-500"
            />
            <select
              value={role}
              onChange={(e) => setRole(e.target.value)}
              className="h-10 rounded-xl border border-slate-200 bg-white px-3 text-sm text-slate-700 outline-none transition focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 dark:border-[#2a2d35] dark:bg-[#1a1d25] dark:text-slate-200"
            >
              <option value="member">Member</option>
              <option value="admin">Admin</option>
            </select>
            <button
              type="submit"
              disabled={inviting}
              className="inline-flex h-10 items-center gap-2 rounded-xl bg-brand-600 px-4 text-sm font-semibold text-white transition hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {inviting ? <Loader2 size={14} className="animate-spin" /> : <UserPlus size={14} />}
              Invite
            </button>
          </div>

          <ErrorMessage error={inviteError} />
        </form>

        {inviteResult && (
          <div className={`mt-4 rounded-xl border p-4 ${inviteResult.is_new_user ? 'border-amber-200 bg-amber-50 dark:border-amber-900/40 dark:bg-amber-950/20' : 'border-green-200 bg-green-50 dark:border-green-900/40 dark:bg-green-950/20'}`}>
            {inviteResult.is_new_user ? (
              <>
                <p className="text-sm font-semibold text-amber-800 dark:text-amber-300 mb-2">
                  New account created — share these credentials
                </p>
                <div className="space-y-1.5 text-sm">
                  <div className="flex items-center">
                    <span className="w-20 text-amber-700 dark:text-amber-400 font-medium">Email</span>
                    <code className="text-amber-900 dark:text-amber-200">{inviteResult.email}</code>
                    <CopyButton text={inviteResult.email} />
                  </div>
                  <div className="flex items-center">
                    <span className="w-20 text-amber-700 dark:text-amber-400 font-medium">Password</span>
                    <code className="text-amber-900 dark:text-amber-200">
                      {showPassword ? inviteResult.password : '••••••••••••••••'}
                    </code>
                    <button
                      onClick={() => setShowPassword((v) => !v)}
                      className="ml-1.5 inline-flex items-center justify-center w-6 h-6 rounded text-amber-600 hover:text-amber-800 dark:text-amber-400 dark:hover:text-amber-200 transition-colors"
                      title={showPassword ? 'Hide password' : 'Show password'}
                    >
                      {showPassword ? <EyeOff size={13} /> : <Eye size={13} />}
                    </button>
                    {inviteResult.password && <CopyButton text={inviteResult.password} />}
                  </div>
                </div>
              </>
            ) : (
              <p className="text-sm text-green-800 dark:text-green-300">
                <span className="font-semibold">{inviteResult.email}</span> has been added to this merchant.
              </p>
            )}
          </div>
        )}
      </div>

      {/* Members list */}
      <div className="rounded-2xl border border-slate-200 bg-white dark:border-[#1d2029] dark:bg-[#0c0e14]">
        <div className="flex items-center gap-2.5 border-b border-slate-100 px-6 py-4 dark:border-[#1a1d25]">
          <Users size={18} className="text-slate-400" />
          <h2 className="text-base font-semibold text-slate-900 dark:text-white">Current members</h2>
          {!loadingMembers && (
            <span className="ml-auto text-xs text-slate-400">{members.length} member{members.length !== 1 ? 's' : ''}</span>
          )}
        </div>

        {loadingMembers ? (
          <div className="flex items-center justify-center py-12 text-slate-400">
            <Loader2 size={20} className="animate-spin" />
          </div>
        ) : membersError ? (
          <div className="px-6 py-8 text-center text-sm text-red-500">{membersError}</div>
        ) : members.length === 0 ? (
          <div className="px-6 py-8 text-center text-sm text-slate-400">No members yet</div>
        ) : (
          <ul className="divide-y divide-slate-100 dark:divide-[#1a1d25]">
            {members.map((m) => (
              <li key={m.user_id} className="flex items-center gap-4 px-6 py-3.5">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-brand-600 text-[11px] font-semibold text-white">
                  {m.email.slice(0, 2).toUpperCase()}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-slate-800 dark:text-slate-200">
                    {m.email}
                  </p>
                  <p className="truncate text-xs text-slate-400">{m.user_id}</p>
                </div>
                <RoleBadge role={m.role} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}
