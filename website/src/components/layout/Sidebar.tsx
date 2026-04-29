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
  ChevronDown,
  LogOut,
} from 'lucide-react'
import { useAuthStore } from '../../store/authStore'
import { apiFetch } from '../../lib/api'

export function Sidebar() {
  const location = useLocation()
  const navigate = useNavigate()
  const { user, clearAuth } = useAuthStore()
  const [pendingPath, setPendingPath] = useState<string | null>(null)
  const [isDark, setIsDark] = useState(() => localStorage.getItem('theme') !== 'light')
  const [accountOpen, setAccountOpen] = useState(false)
  const accountRef = useRef<HTMLDivElement>(null)
  const selectedPath = pendingPath ?? location.pathname
  const assetBaseUrl = import.meta.env.BASE_URL
  const initials = user?.email ? user.email.slice(0, 2).toUpperCase() : 'ME'

  useEffect(() => {
    const root = window.document.documentElement
    if (isDark) {
      root.classList.add('dark')
      localStorage.setItem('theme', 'dark')
    } else {
      root.classList.remove('dark')
      localStorage.setItem('theme', 'light')
    }
  }, [isDark])

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (accountRef.current && !accountRef.current.contains(event.target as Node)) {
        setAccountOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
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
        <div className="flex items-center">
          <img
            src={`${assetBaseUrl}logo/decision-engine-light.svg`}
            alt="Juspay Decision Engine"
            className="h-11 w-auto dark:hidden"
          />
          <img
            src={`${assetBaseUrl}logo/decision-engine-dark.svg`}
            alt="Juspay Decision Engine"
            className="hidden h-11 w-auto dark:block"
          />
        </div>
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
            onClick={() => setIsDark((value) => !value)}
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
