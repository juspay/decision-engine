import { NavLink } from 'react-router-dom'
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
  return (
    <aside className="w-64 shrink-0 flex flex-col h-screen bg-white dark:bg-black border-r border-slate-200 dark:border-[#151515] relative z-20 transition-colors duration-300">
      {/* Logo */}
      <div className="min-h-20 px-6 py-5 flex flex-col justify-center gap-2 border-b border-slate-200 dark:border-[#151515] transition-colors duration-300">
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
      <nav className="flex-1 px-4 py-8 space-y-1 overflow-y-auto">
        <SideLink to="/" icon={LayoutDashboard} end>Overview</SideLink>
        <SideLink to="/decisions" icon={Search}>Decision Explorer</SideLink>
        <SideLink to="/analytics" icon={BarChart3}>Analytics</SideLink>
        <SideLink to="/audit" icon={Activity}>Payment Audit</SideLink>

        <div className="pt-8 pb-3 px-3 flex items-center gap-2">
          <span className="text-[11px] font-bold uppercase tracking-widest text-slate-400 dark:text-[#66666e]">
            Routing
          </span>
        </div>

        <SideLink to="/routing" icon={GitBranch} end>Routing Hub</SideLink>
        <SideLink to="/routing/sr" icon={TrendingUp} indent>Auth-Rate Based</SideLink>
        <SideLink to="/routing/rules" icon={BookOpen} indent>Rule-Based (Euclid)</SideLink>
        <SideLink to="/routing/volume" icon={PieChart} indent>Volume Split</SideLink>
        <SideLink to="/routing/debit" icon={Network} indent>Debit Routing</SideLink>
      </nav>

      {/* Footer */}
      <div className="px-6 py-5 border-t border-slate-200 dark:border-[#151515] bg-slate-50 dark:bg-black transition-colors duration-300">
        <span className="text-[11px] text-slate-500 dark:text-[#66666e] font-medium tracking-wide">v1.4</span>
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
}: {
  to: string
  icon: React.ElementType
  children: React.ReactNode
  end?: boolean
  indent?: boolean
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        `group relative flex items-center gap-3 px-4 py-3 rounded-[14px] text-[14px] font-medium transition-all duration-200 ${indent ? 'ml-3 w-[calc(100%-12px)]' : ''
        } ${isActive
          ? 'bg-slate-100 text-brand-600 dark:bg-[#151518] dark:text-white shadow-sm'
          : 'text-slate-500 hover:text-slate-900 hover:bg-slate-50 dark:text-[#888891] dark:hover:text-white dark:hover:bg-[#0c0c0e]'
        }`
      }
    >
      {({ isActive }) => (
        <>
          <Icon
            size={18}
            className={`transition-colors duration-200 ${isActive ? 'text-brand-600 dark:text-white' : 'text-slate-400 dark:text-[#55555e] group-hover:text-slate-600 dark:group-hover:text-white'}`}
            strokeWidth={isActive ? 2.5 : 2}
          />
          <span className="flex-1">{children}</span>
        </>
      )}
    </NavLink>
  )
}
