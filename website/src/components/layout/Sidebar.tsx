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
    <aside className="w-56 shrink-0 flex flex-col h-screen bg-[#060609] border-r border-[#14141c]">
      {/* Logo */}
      <div className="h-14 px-5 flex items-center gap-2.5 border-b border-[#14141c]">
        <div className="w-7 h-7 bg-brand-500 rounded-lg flex items-center justify-center shadow-[0_0_12px_rgba(22,104,227,0.4)]">
          <Zap size={14} className="text-white" strokeWidth={2.5} />
        </div>
        <div>
          <p className="text-[11px] font-semibold tracking-[0.12em] uppercase text-white leading-none">
            Decision
          </p>
          <p className="text-[10px] tracking-[0.1em] uppercase text-gray-500 leading-none mt-0.5">
            Engine
          </p>
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-2.5 py-4 space-y-0.5 overflow-y-auto">
        <SideLink to="/" icon={LayoutDashboard} end>Overview</SideLink>
        <SideLink to="/decisions" icon={Search}>Decision Explorer</SideLink>

        <div className="pt-4 pb-1.5 px-3">
          <span className="text-[10px] font-semibold tracking-[0.12em] uppercase text-gray-500">
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
      <div className="px-5 py-3 border-t border-[#14141c]">
        <span className="text-[10px] text-gray-500 tracking-widest font-mono">v1.2.1</span>
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
        `group relative flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all duration-100 ${
          indent ? 'ml-2 pl-3' : ''
        } ${
          isActive
            ? 'bg-[#10101a] text-white'
            : 'text-gray-500 hover:text-gray-800 hover:bg-[#0d0d14]'
        }`
      }
    >
      {({ isActive }) => (
        <>
          {isActive && (
            <span className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-4 bg-brand-500 rounded-full" />
          )}
          <Icon
            size={13}
            className={isActive ? 'text-brand-500' : 'text-gray-500 group-hover:text-gray-600'}
            strokeWidth={2}
          />
          <span className="flex-1 tracking-wide">{children}</span>
        </>
      )}
    </NavLink>
  )
}
