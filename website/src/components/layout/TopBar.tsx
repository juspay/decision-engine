import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ChevronDown, LogOut, Moon, Sun } from 'lucide-react'
import { apiFetch } from '../../lib/api'
import { useAuthStore } from '../../store/authStore'

export function TopBar() {
  const navigate = useNavigate()
  const { user, clearAuth } = useAuthStore()
  const [isDark, setIsDark] = useState(() => localStorage.getItem('theme') === 'dark')

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

  async function handleLogout() {
    try {
      await apiFetch('/auth/logout', { method: 'POST' })
    } catch {
      // best-effort - clear locally regardless
    }
    clearAuth()
    navigate('/login', { replace: true })
  }

  const initials = user?.email ? user.email.slice(0, 2).toUpperCase() : 'ME'

  return (
    <header className="h-14 bg-white dark:bg-[#0c0c10] border-b border-[#e6e6ee] dark:border-[#1a1a24] flex items-center justify-between px-6 shrink-0 relative z-10">
      <div />

      <div className="flex items-center gap-2">
        <button
          onClick={() => setIsDark(!isDark)}
          className="w-8 h-8 flex items-center justify-center rounded-lg text-slate-500 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-[#1a1a24] transition-colors"
          aria-label="Toggle theme"
        >
          {isDark ? <Sun size={16} /> : <Moon size={16} />}
        </button>

        <div className="w-px h-5 bg-[#e6e6ee] dark:bg-[#1a1a24] mx-1" />

        {user && (
          <div className="flex items-center gap-2 pl-1">
            <div className="w-7 h-7 rounded-full bg-brand-600 flex items-center justify-center">
              <span className="text-[10px] font-semibold text-white">{initials}</span>
            </div>
            <div className="hidden sm:block">
              <p className="text-[13px] font-medium text-slate-700 dark:text-slate-300 leading-tight">
                {user.email}
              </p>
              {user.merchantId && (
                <p className="text-[11px] text-slate-400 dark:text-slate-500 leading-tight">
                  {user.merchantId}
                </p>
              )}
            </div>
            <ChevronDown size={14} className="text-slate-400 dark:text-slate-500 ml-0.5" />
          </div>
        )}

        <button
          onClick={handleLogout}
          className="w-8 h-8 flex items-center justify-center rounded-lg text-slate-500 hover:bg-red-50 hover:text-red-500 dark:text-slate-400 dark:hover:bg-red-950/30 dark:hover:text-red-400 transition-colors ml-1"
          aria-label="Sign out"
          title="Sign out"
        >
          <LogOut size={16} />
        </button>
      </div>
    </header>
  )
}
