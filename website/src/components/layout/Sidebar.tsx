import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import {
  LayoutDashboard,
  GitBranch,
  Search,
  TrendingUp,
  BookOpen,
  PieChart,
  Network,
  BarChart3,
  Activity,
  Moon,
  Sun,
  Users,
  Key,
  ChevronDown,
  LogOut,
} from 'lucide-react'
import { useAuthStore } from '../../store/authStore'
import { apiFetch } from '../../lib/api'
import {
  applyThemePreference,
  getResolvedThemePreference,
  getStoredThemePreference,
  persistThemePreference,
} from '../../lib/theme'

export function Sidebar() {
  const location = useLocation()
  const navigate = useNavigate()
  const { user, clearAuth } = useAuthStore()
  const [pendingPath, setPendingPath] = useState<string | null>(null)
  const [isDark, setIsDark] = useState(() => getResolvedThemePreference() === 'dark')
  const [accountOpen, setAccountOpen] = useState(false)
  const accountRef = useRef<HTMLDivElement>(null)
  const selectedPath = pendingPath ?? location.pathname
  const initials = user?.email ? user.email.slice(0, 2).toUpperCase() : 'ME'

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (accountRef.current && !accountRef.current.contains(event.target as Node)) {
        setAccountOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  useEffect(() => {
    if (typeof window.matchMedia !== 'function') {
      return
    }

    const systemTheme = window.matchMedia('(prefers-color-scheme: dark)')
    const syncSystemTheme = () => {
      if (getStoredThemePreference()) {
        return
      }

      const nextIsDark = systemTheme.matches
      setIsDark(nextIsDark)
      applyThemePreference(nextIsDark ? 'dark' : 'light')
    }

    syncSystemTheme()
    systemTheme.addEventListener('change', syncSystemTheme)
    return () => systemTheme.removeEventListener('change', syncSystemTheme)
  }, [])

  async function handleLogout() {
    setAccountOpen(false)
    try {
      await apiFetch('/auth/logout', { method: 'POST' })
    } catch {
      // Clear local auth even if the server-side logout call fails.
    }
    clearAuth()
    navigate('/login', { replace: true })
  }

  function handleThemeToggle() {
    const nextTheme = isDark ? 'light' : 'dark'
    setIsDark(nextTheme === 'dark')
    persistThemePreference(nextTheme)
  }

  useLayoutEffect(() => {
    if (!pendingPath) {
      return
    }

    const navigationSettled =
      location.pathname === pendingPath ||
      location.pathname.startsWith(`${pendingPath}/`)

    if (navigationSettled) {
      setPendingPath(null)
    }
  }, [location.pathname, pendingPath])

  return (
    <aside className="relative z-20 flex h-screen w-64 shrink-0 flex-col border-r border-slate-200 bg-white transition-colors duration-300 dark:border-[#22262f] dark:bg-[#06080d]">
      {/* Logo */}
      <div className="flex h-[78px] shrink-0 items-center border-b border-slate-200 px-6 transition-colors duration-300 dark:border-[#22262f]">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 480 56"
          fill="none"
          className="h-14 w-auto"
          aria-label="Juspay Decision Engine"
          role="img"
        >
          <defs>
            <style>{`.de-brand{font-family:Inter,Arial,sans-serif;font-size:30px;dominant-baseline:central}.de-bold{font-weight:700;letter-spacing:-0.02em}.de-semi{font-weight:600;letter-spacing:-0.01em}`}</style>
          </defs>
          <g transform="translate(0, 1)">
            <path fillRule="evenodd" clipRule="evenodd" d="M27.2375 54.3079C25.9366 54.3441 24.6718 54.2355 23.3708 54.0545C19.8655 53.5477 16.577 52.4255 13.5414 50.5792C6.81987 46.5246 2.51952 40.6238 0.748782 32.8766C0.35127 31.139 0.170583 29.3651 0.170583 27.5912C0.134446 20.4957 2.44724 14.2691 7.1451 8.98363C11.2286 4.42224 16.2879 1.63472 22.2505 0.476271C23.9129 0.150457 25.5752 -0.0305508 27.2375 0.0418522C27.2375 2.75697 27.2375 5.43588 27.2375 8.15099C27.1652 8.18719 27.093 8.2234 27.0207 8.2958C26.045 8.98363 25.0693 9.70766 24.1297 10.4679C21.9976 12.2056 19.9739 14.0518 18.2754 16.2601C15.9988 19.1925 14.481 22.4144 14.2642 26.2156C14.1558 28.0256 14.3726 29.7995 14.9508 31.501C16.3601 35.7366 20.7328 40.1169 26.8761 40.2255C27.2375 40.2255 27.3098 40.298 27.3098 40.6962C27.2736 42.5424 27.2736 46.235 27.2736 46.235C27.2736 46.235 27.2736 46.4522 27.2736 46.5608C27.2375 49.1311 27.2375 51.7014 27.2375 54.3079Z" fill="#0099FF" />
            <path fillRule="evenodd" clipRule="evenodd" d="M27.2368 8.11469C27.2368 8.11469 27.2368 2.72066 27.2368 0.00554404C28.7184 -0.0306575 30.1639 0.114149 31.6094 0.331358C34.3197 0.765776 36.9216 1.56221 39.379 2.82926C44.4743 5.47197 48.4856 9.23693 51.1959 14.3775C52.7498 17.3461 53.7617 20.4956 54.1592 23.7899C54.6289 27.6635 54.2676 31.5009 53.1112 35.2296C51.7741 39.4652 49.5336 43.194 46.3896 46.3435C42.5591 50.1447 38.0057 52.6064 32.7297 53.7286C30.9228 54.0906 29.0798 54.3078 27.2368 54.2716C27.2368 51.6651 27.2368 49.0948 27.2368 46.4883C27.2368 46.3797 27.2368 46.1987 27.2368 46.1987C27.2368 46.1987 27.3452 46.1263 27.3813 46.0901C30.2001 44.099 32.8742 41.9269 35.187 39.3204C36.7048 37.6189 38.0057 35.8088 38.9092 33.7092C40.174 30.7406 40.6799 27.6997 40.0294 24.4778C38.873 18.6131 33.6331 14.2327 27.5982 14.0879C27.2007 14.0879 27.2368 13.7621 27.2368 13.7621V8.11469Z" fill="#0561E2" />
          </g>
          <text x="72" y="28" className="de-brand de-bold fill-[#0F172A] dark:fill-white">JUSPAY</text>
          <text x="200" y="28" className="de-brand de-semi fill-[#475569] dark:fill-[#9CA3AF]">Decision Engine</text>
        </svg>
      </div>

      {/* Nav */}
      <nav className="flex-1 space-y-1 overflow-y-auto px-4 py-8">
        <SideLink to="/" icon={LayoutDashboard} end selectedPath={selectedPath} onNavigate={setPendingPath}>Overview</SideLink>
        <SideLink to="/analytics" icon={BarChart3} selectedPath={selectedPath} onNavigate={setPendingPath}>Analytics</SideLink>
        <SideLink to="/audit" icon={Activity} selectedPath={selectedPath} onNavigate={setPendingPath}>Decision Audit</SideLink>

        <div className="flex items-center gap-2 px-3 pb-3 pt-8">
          <span className="text-[11px] font-bold uppercase tracking-widest text-slate-400 dark:text-[#6d768a]">
            Routing
          </span>
        </div>

        <SideLink to="/routing" icon={GitBranch} end selectedPath={selectedPath} onNavigate={setPendingPath}>Routing Hub</SideLink>
        <SideLink to="/routing/sr" icon={TrendingUp} indent selectedPath={selectedPath} onNavigate={setPendingPath}>Auth-Rate Based</SideLink>
        <SideLink to="/routing/rules" icon={BookOpen} indent selectedPath={selectedPath} onNavigate={setPendingPath}>Rule-Based</SideLink>
        <SideLink to="/routing/volume" icon={PieChart} indent selectedPath={selectedPath} onNavigate={setPendingPath}>Volume Split</SideLink>
        <SideLink to="/routing/debit" icon={Network} indent selectedPath={selectedPath} onNavigate={setPendingPath}>Debit Routing</SideLink>

        <div className="flex items-center gap-2 px-3 pb-3 pt-8">
          <span className="text-[11px] font-bold uppercase tracking-widest text-slate-400 dark:text-[#6d768a]">
            Simulation
          </span>
        </div>

        <SideLink to="/decisions" icon={Search} selectedPath={selectedPath} onNavigate={setPendingPath}>Decision Explorer</SideLink>

        <div className="flex items-center gap-2 px-3 pb-3 pt-8">
          <span className="text-[11px] font-bold uppercase tracking-widest text-slate-400 dark:text-[#6d768a]">
            Settings
          </span>
        </div>

        <SideLink to="/members" icon={Users} selectedPath={selectedPath} onNavigate={setPendingPath}>Members</SideLink>
        <SideLink to="/api-keys" icon={Key} selectedPath={selectedPath} onNavigate={setPendingPath}>API Keys</SideLink>
      </nav>

      <div ref={accountRef} className="relative border-t border-slate-200 bg-white px-4 py-4 transition-colors duration-300 dark:border-[#22262f] dark:bg-[#0a0d12]">
        {accountOpen ? (
          <div className="absolute bottom-full left-4 right-4 mb-3 overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-[0_20px_60px_-36px_rgba(15,23,42,0.35)] dark:border-[#2a303a] dark:bg-[#0d1118]">
            <div className="border-b border-slate-200 px-3 py-3 dark:border-[#242b36]">
              <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                {user?.email || 'Signed-in user'}
              </p>
              {user?.merchantId ? (
                <p className="mt-1 truncate text-xs text-slate-500 dark:text-[#8a8a93]">
                  {user.merchantId}
                </p>
              ) : null}
            </div>
            <button
              type="button"
              onClick={handleLogout}
              className="flex w-full items-center gap-2.5 px-3 py-3 text-left text-sm font-medium text-red-600 transition-colors hover:bg-red-50 dark:text-red-300 dark:hover:bg-red-950/25"
            >
              <LogOut size={16} />
              Logout
            </button>
          </div>
        ) : null}

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setAccountOpen((value) => !value)}
            aria-haspopup="menu"
            aria-expanded={accountOpen}
            className="flex min-w-0 flex-1 items-center gap-3 rounded-2xl px-2 py-2 text-left transition-colors hover:bg-slate-100 dark:hover:bg-[#151b24]"
          >
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-brand-600 text-[11px] font-semibold text-white">
              {initials}
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm font-semibold text-slate-900 dark:text-white">
                {user?.email || 'Account'}
              </span>
            </span>
            <ChevronDown
              size={15}
              className={`shrink-0 text-slate-400 transition-transform ${accountOpen ? 'rotate-180' : ''}`}
            />
          </button>

          <button
            type="button"
            onClick={handleThemeToggle}
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-950 dark:text-slate-400 dark:hover:bg-[#1a1f2a] dark:hover:text-white"
            aria-label="Toggle theme"
            title="Toggle theme"
          >
            {isDark ? <Sun size={18} /> : <Moon size={18} />}
          </button>
        </div>
      </div>
    </aside>
  )
}

function SideLink({
  to,
  icon: Icon,
  children,
  end,
  indent,
  selectedPath,
  onNavigate,
}: {
  to: string
  icon: React.ElementType
  children: React.ReactNode
  end?: boolean
  indent?: boolean
  selectedPath: string
  onNavigate?: (path: string) => void
}) {
  const navigate = useNavigate()
  const isHighlighted = end
    ? selectedPath === to
    : selectedPath === to || selectedPath.startsWith(`${to}/`)

  return (
    <button
      type="button"
      aria-current={isHighlighted ? 'page' : undefined}
      onMouseDown={(event) => {
        if (event.detail > 0) {
          event.preventDefault()
        }
      }}
      onClick={(event) => {
        if (document.activeElement instanceof HTMLElement) {
          document.activeElement.blur()
        }
        onNavigate?.(to)
        event.currentTarget.blur()
        navigate(to)
      }}
      className={`group relative flex w-full appearance-none items-center gap-3 rounded-[16px] border-0 px-4 py-3 text-[14px] font-medium transition-colors duration-150 focus:outline-none focus-visible:ring-2 focus-visible:ring-[#3b82f6]/40 focus-visible:ring-offset-0 ${indent ? 'pl-8' : ''
        } ${isHighlighted
          ? 'bg-transparent text-slate-950 dark:text-white'
          : 'bg-transparent text-slate-500 hover:bg-slate-900/[0.025] hover:text-slate-900 dark:text-[#8d96aa] dark:hover:bg-white/[0.035] dark:hover:text-white'
        }`}
    >
      <span
        aria-hidden="true"
        className={`absolute left-1 top-1/2 h-7 w-[3px] -translate-y-1/2 rounded-full transition-all duration-150 ${isHighlighted ? 'bg-brand-600 opacity-100 dark:bg-sky-300' : 'opacity-0'}`}
      />
      <Icon
        size={18}
        className={`transition-colors duration-200 ${isHighlighted ? 'text-brand-600 dark:text-sky-300' : 'text-slate-400 group-hover:text-slate-700 dark:text-[#697387] dark:group-hover:text-white'}`}
        strokeWidth={isHighlighted ? 2.5 : 2}
      />
      <span className="flex-1 text-left">{children}</span>
    </button>
  )
}
