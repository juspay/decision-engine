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
    <aside className="w-64 shrink-0 flex flex-col h-screen bg-black border-r border-[#151515] relative z-20">
      {/* Logo */}
      <div className="h-20 px-6 flex items-center gap-3.5 border-b border-[#151515]">
        <div className="w-9 h-9 bg-black border border-[#2a2a2e] rounded-xl flex items-center justify-center shadow-sm">
          <Zap size={18} className="text-white" strokeWidth={2} />
        </div>
        <div>
          <p className="text-[16px] font-bold tracking-widest text-white leading-tight">
            MODE
          </p>
          <p className="text-[10px] uppercase text-[#66666e] tracking-wider leading-tight">
            Decision Engine
          </p>
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-4 py-6 space-y-1 overflow-y-auto">
        <SideLink to="/" icon={LayoutDashboard} end>Overview</SideLink>
        <SideLink to="/decisions" icon={Search}>Decision Explorer</SideLink>

        <div className="pt-6 pb-2 px-3 flex items-center gap-2">
          <span className="text-[11px] font-semibold tracking-wider text-[#66666e]">
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
      <div className="px-6 py-4 border-t border-[#151515] bg-black">
        <span className="text-[11px] text-[#66666e] font-medium">v1.0.1 Black</span>
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
        `group relative flex items-center gap-3 px-4 py-2.5 rounded-full text-[13px] font-medium transition-all duration-200 ${indent ? 'ml-3 w-[calc(100%-12px)]' : ''
        } ${isActive
          ? 'bg-[#151518] text-white shadow-sm'
          : 'text-[#888891] hover:text-white hover:bg-[#0c0c0e]'
        }`
      }
    >
      {({ isActive }) => (
        <>
          <Icon
            size={16}
            className={`transition-colors duration-200 ${isActive ? 'text-white' : 'text-[#55555e] group-hover:text-white'}`}
            strokeWidth={2}
          />
          <span className="flex-1">{children}</span>
        </>
      )}
    </NavLink>
  )
}
