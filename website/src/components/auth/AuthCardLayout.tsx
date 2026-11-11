import { useState } from 'react'
import type { LucideIcon } from 'lucide-react'
import { Moon, Sun } from 'lucide-react'
import { getResolvedThemePreference, persistThemePreference } from '../../lib/theme'

/** The primary call-to-action style shared by the signed-out pages. */
export const AUTH_PRIMARY_BUTTON =
  'inline-flex h-12 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-6 text-sm font-semibold text-white transition-all hover:brightness-110'

function ThemeToggleButton() {
  const [isDark, setIsDark] = useState(() => getResolvedThemePreference() === 'dark')

  function toggle() {
    const next = isDark ? 'light' : 'dark'
    setIsDark(next === 'dark')
    persistThemePreference(next)
  }

  return (
    <button
      type="button"
      onClick={toggle}
      className="flex h-9 w-9 items-center justify-center rounded-xl text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-950 dark:text-slate-400 dark:hover:bg-white/8 dark:hover:text-white"
      aria-label="Toggle theme"
      title="Toggle theme"
    >
      {isDark ? <Sun size={18} /> : <Moon size={18} />}
    </button>
  )
}

function BrandLogo() {
  const assetBaseUrl = import.meta.env.BASE_URL
  return (
    <div>
      <img
        src={`${assetBaseUrl}logo/decision-engine-light.svg`}
        alt="Juspay Decision Engine"
        className="h-10 w-auto dark:hidden sm:h-11"
      />
      <img
        src={`${assetBaseUrl}logo/decision-engine-dark.svg`}
        alt="Juspay Decision Engine"
        className="hidden h-10 w-auto dark:block sm:h-11"
      />
    </div>
  )
}

const CARD_SIZES = {
  // A form: title, fields and a submit button.
  form: 'max-w-[440px] px-8 py-10 sm:px-10',
  // A single status message with an icon, centred.
  status: 'max-w-[420px] px-10 py-12 text-center',
}

/**
 * The signed-out page frame: backdrop, logo and theme toggle, one centred card, footer.
 * Used by the forgot-password, reset-password and verify-email pages.
 */
export function AuthCardLayout({
  size = 'form',
  children,
}: {
  size?: keyof typeof CARD_SIZES
  children: React.ReactNode
}) {
  return (
    <div className="relative min-h-screen overflow-hidden bg-white text-slate-900 dark:bg-[#030507] dark:text-white">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_18%_18%,_rgba(59,130,246,0.06),_transparent_24%),radial-gradient(circle_at_78%_20%,_rgba(14,165,233,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.03),_transparent_24%)] dark:bg-[radial-gradient(circle_at_18%_18%,_rgba(56,189,248,0.05),_transparent_24%),radial-gradient(circle_at_78%_20%,_rgba(59,130,246,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.035),_transparent_24%)]" />
      <div className="pointer-events-none absolute inset-0 opacity-[0.05] dark:opacity-[0.08] [background-image:linear-gradient(rgba(148,163,184,0.08)_1px,transparent_1px),linear-gradient(90deg,rgba(148,163,184,0.08)_1px,transparent_1px)] [background-size:56px_56px]" />

      <div className="relative z-10 flex min-h-screen flex-col">
        <header className="flex items-center justify-between px-6 py-5 sm:px-10">
          <BrandLogo />
          <ThemeToggleButton />
        </header>

        <div className="flex flex-1 items-center justify-center px-6 py-12">
          <div className={`w-full rounded-3xl border border-slate-200 bg-white shadow-[0_20px_60px_-20px_rgba(15,23,42,0.08)] dark:border-[#1d1d23] dark:bg-[#0b0e14] dark:shadow-none ${CARD_SIZES[size]}`}>
            {children}
          </div>
        </div>

        <footer className="px-6 py-5 text-center text-xs text-slate-500 dark:text-[#78849a]">
          Juspay Decision Engine
        </footer>
      </div>
    </div>
  )
}

const STATUS_TONES = {
  progress: { ring: 'bg-brand-500/10', icon: 'text-brand-600' },
  success: { ring: 'bg-emerald-500/10', icon: 'text-emerald-700' },
  error: { ring: 'bg-red-500/10', icon: 'text-red-600' },
}

/** An icon in a tinted circle over a title and a line of text, with an optional action below. */
export function AuthStatus({
  icon: Icon,
  tone,
  spin = false,
  title,
  children,
  action,
}: {
  icon: LucideIcon
  tone: keyof typeof STATUS_TONES
  spin?: boolean
  title: React.ReactNode
  children: React.ReactNode
  action?: React.ReactNode
}) {
  const { ring, icon } = STATUS_TONES[tone]
  return (
    <div className="space-y-5">
      <div className={`mx-auto flex h-16 w-16 items-center justify-center rounded-full ${ring}`}>
        <Icon size={28} className={`${icon} ${spin ? 'animate-spin' : ''}`} />
      </div>
      <div className="space-y-2">
        <p className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">{title}</p>
        <p className="text-sm text-slate-500 dark:text-[#8a94a7]">{children}</p>
      </div>
      {action}
    </div>
  )
}
