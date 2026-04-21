import { NavLink } from 'react-router-dom'
import {
  LayoutDashboard,
  GitBranch,
  Search,
  TrendingUp,
  BookOpen,
  PieChart,
  Network,
} from 'lucide-react'

export function Sidebar() {
  return (
    <aside className="w-[260px] shrink-0 flex flex-col h-screen bg-white dark:bg-[#0c0c10] border-r border-[#e6e6ee] dark:border-[#1a1a24] relative z-20">
      {/* Logo */}
      <div className="h-16 px-5 flex items-center gap-3 border-b border-[#e6e6ee] dark:border-[#1a1a24]">
        <img src="/dashboard/hyperswitch-icon.png" alt="Hyperswitch" className="w-8 h-8 rounded-lg" />
        <div>
          <p className="text-[13px] font-semibold text-slate-800 dark:text-white leading-tight tracking-tight">
            Decision Engine
          </p>
          <p className="text-[11px] text-slate-400 dark:text-slate-500 leading-tight">
            by Juspay
          </p>
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-3 py-4 overflow-y-auto">
        <SideLink to="/" icon={LayoutDashboard} end>Overview</SideLink>
        <SideLink to="/decisions" icon={Search}>Decision Explorer</SideLink>

        <div className="mt-5 mb-2 px-3">
          <span className="text-[10px] font-semibold uppercase tracking-widest text-slate-400 dark:text-slate-600">
            Routing
          </span>
        </div>

        <SideLink to="/routing" icon={GitBranch} end>Routing Hub</SideLink>
        <SideLink to="/routing/sr" icon={TrendingUp}>Auth-Rate Based</SideLink>
        <SideLink to="/routing/rules" icon={BookOpen}>Rule-Based (Euclid)</SideLink>
        <SideLink to="/routing/volume" icon={PieChart}>Volume Split</SideLink>
        <SideLink to="/routing/debit" icon={Network}>Debit Routing</SideLink>
      </nav>

      {/* Footer */}
      <div className="px-5 py-3 border-t border-[#e6e6ee] dark:border-[#1a1a24]">
        <span className="text-[11px] text-slate-400 dark:text-slate-600">v1.4</span>
      </div>
    </aside>
  )
}


function SideLink({
  to,
  icon: Icon,
  children,
  end,
}: {
  to: string
  icon: React.ElementType
  children: React.ReactNode
  end?: boolean
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        `group flex items-center gap-2.5 px-3 py-2 rounded-lg text-[13px] font-medium transition-colors duration-150 mb-0.5 ${
          isActive
            ? 'bg-[#eff6ff] dark:bg-[#0d1f3c] text-brand-600 dark:text-brand-500'
            : 'text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-[#13131a] hover:text-slate-800 dark:hover:text-slate-200'
        }`
      }
    >
      {({ isActive }) => (
        <>
          <Icon
            size={16}
            className={`shrink-0 transition-colors duration-150 ${
              isActive
                ? 'text-brand-600 dark:text-brand-500'
                : 'text-slate-400 dark:text-slate-500 group-hover:text-slate-600 dark:group-hover:text-slate-300'
            }`}
            strokeWidth={isActive ? 2.5 : 2}
          />
          <span className="flex-1 leading-none">{children}</span>
        </>
      )}
    </NavLink>
  )
}
