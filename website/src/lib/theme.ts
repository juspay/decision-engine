export type ThemePreference = 'light' | 'dark'

const THEME_STORAGE_KEY = 'theme'

export function getStoredThemePreference(): ThemePreference | null {
  if (typeof window === 'undefined') {
    return null
  }

  const storedTheme = window.localStorage.getItem(THEME_STORAGE_KEY)
  return storedTheme === 'dark' || storedTheme === 'light' ? storedTheme : null
}

export function getSystemThemePreference(): ThemePreference {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return 'light'
  }

  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

export function getResolvedThemePreference(): ThemePreference {
  return getStoredThemePreference() ?? getSystemThemePreference()
}

export function applyThemePreference(theme: ThemePreference = getResolvedThemePreference()) {
  if (typeof document === 'undefined') {
    return
  }

  document.documentElement.classList.toggle('dark', theme === 'dark')
}

export function persistThemePreference(theme: ThemePreference) {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme)
  }

  applyThemePreference(theme)
}
