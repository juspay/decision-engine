import { Outlet } from 'react-router-dom'
import { Sidebar } from './Sidebar'
import { TopBar } from './TopBar'

export function AppShell() {
  return (
    <div className="flex h-screen overflow-hidden bg-black text-white relative">
      <div className="aurora-top" />
      <Sidebar />
      <div className="flex-1 flex flex-col overflow-hidden relative z-10">
        <TopBar />
        <main className="flex-1 overflow-y-auto p-8 relative">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
