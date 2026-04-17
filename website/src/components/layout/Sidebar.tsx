import { useLayoutEffect, useState } from 'react'
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
} from 'lucide-react'

export function Sidebar() {
  const location = useLocation()
  const [pendingPath, setPendingPath] = useState<string | null>(null)
  const selectedPath = pendingPath ?? location.pathname

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
      <div className="flex min-h-20 flex-col justify-center gap-2 border-b border-slate-200 px-6 py-5 transition-colors duration-300 dark:border-[#22262f]">
        <div className="flex items-center">
          <img
            src="/dashboard/logo/decision-engine-light.svg"
            alt="Juspay Decision Engine"
            className="h-11 w-auto dark:hidden"
          />
          <img
            src="/dashboard/logo/decision-engine-dark.svg"
            alt="Juspay Decision Engine"
            className="hidden h-11 w-auto dark:block"
          />
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 space-y-1 overflow-y-auto px-4 py-8">
        <SideLink to="/" icon={LayoutDashboard} end selectedPath={selectedPath} onNavigate={setPendingPath}>Overview</SideLink>
        <SideLink to="/decisions" icon={Search} selectedPath={selectedPath} onNavigate={setPendingPath}>Decision Explorer</SideLink>
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
      </nav>

      {/* Footer */}
      <div className="border-t border-slate-200 bg-white px-6 py-5 transition-colors duration-300 dark:border-[#22262f] dark:bg-[#0a0d12]">
        <span className="text-[11px] font-medium tracking-wide text-slate-500 dark:text-[#7d879b]">v1.4</span>
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
      className={`group relative flex w-full appearance-none items-center gap-3 rounded-[16px] border-0 px-4 py-3 text-[14px] font-medium transition-colors duration-150 focus:outline-none focus-visible:ring-2 focus-visible:ring-[#3b82f6]/40 focus-visible:ring-offset-0 ${indent ? 'ml-3 w-[calc(100%-12px)]' : ''
        } ${isHighlighted
          ? 'bg-slate-900/[0.045] text-slate-950 shadow-[inset_0_1px_0_rgba(255,255,255,0.7),0_14px_30px_-24px_rgba(15,23,42,0.45)] dark:bg-[#151922] dark:text-white dark:shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]'
          : 'bg-transparent text-slate-500 hover:bg-slate-900/[0.025] hover:text-slate-900 dark:text-[#8d96aa] dark:hover:bg-white/[0.035] dark:hover:text-white'
        }`}
    >
      <span
        aria-hidden="true"
        className={`absolute left-1 top-1/2 h-8 w-1 -translate-y-1/2 rounded-full transition-all duration-150 ${isHighlighted ? 'bg-brand-600 opacity-100 dark:bg-sky-300' : 'opacity-0'}`}
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
