import { NavLink } from 'react-router-dom'
import {
  LayoutDashboard,
  GitBranch,
  Search,
  Zap,
  TrendingUp,
  BookOpen,
  PieChart,
  Network,
} from 'lucide-react'

export function Sidebar() {
  return (
    <aside className="w-64 shrink-0 flex flex-col h-screen bg-white dark:bg-black border-r border-slate-200 dark:border-[#151515] relative z-20 transition-colors duration-300">
      {/* Logo */}
      <div className="h-20 px-6 flex items-center gap-3.5 border-b border-slate-200 dark:border-[#151515] transition-colors duration-300">
        <div className="w-9 h-9 bg-brand-500 dark:bg-black border border-brand-600 dark:border-[#2a2a2e] rounded-xl flex items-center justify-center shadow-sm">
          <Zap size={18} className="text-white" strokeWidth={2} />
        </div>
        <div>
          <p className="text-[16px] font-bold tracking-widest text-slate-900 dark:text-white leading-tight uppercase">
            Decision
          </p>
          <p className="text-[10px] uppercase text-brand-600 dark:text-[#66666e] tracking-wider leading-tight font-semibold">
            Engine
          </p>
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-4 py-8 space-y-1 overflow-y-auto">
        <SideLink to="/" icon={LayoutDashboard} end>Overview</SideLink>
        <SideLink to="/decisions" icon={Search}>Decision Explorer</SideLink>

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
        <span className="text-[11px] text-slate-500 dark:text-[#66666e] font-medium tracking-wide">v1.2.1</span>
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
