import { Outlet } from 'react-router-dom'
import { Sidebar } from './Sidebar'
import { TopBar } from './TopBar'

export function AppShell() {
  return (
    <div className="relative flex h-screen overflow-hidden bg-[#ffffff] text-slate-900 transition-colors duration-300 dark:bg-[#030507] dark:text-white">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_left,_rgba(59,130,246,0.05),_transparent_22%),radial-gradient(circle_at_top_right,_rgba(14,165,233,0.04),_transparent_20%),linear-gradient(180deg,_rgba(255,255,255,1),_rgba(255,255,255,1))] dark:bg-[radial-gradient(circle_at_top_left,_rgba(56,189,248,0.06),_transparent_22%),linear-gradient(180deg,_rgba(3,5,7,1),_rgba(5,8,12,1))]" />
      <div className="aurora-top" />
      <Sidebar />
      <div className="flex-1 flex flex-col overflow-hidden relative z-10">
        <TopBar />
        <main className="relative flex-1 overflow-y-auto p-6 md:p-8">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
