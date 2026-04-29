export type ThemePreference = 'light' | 'dark'

const THEME_STORAGE_KEY = 'theme'

export function getStoredThemePreference(): ThemePreference {
  if (typeof window === 'undefined') {
    return 'light'
  }

  return window.localStorage.getItem(THEME_STORAGE_KEY) === 'dark' ? 'dark' : 'light'
}

export function applyThemePreference(theme: ThemePreference = getStoredThemePreference()) {
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
