import { isEmbedded } from './embedMode'

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

  // Embedded in the dashboard, the theme is fixed to light and unchangeable: the frame must match
  // the host's white chrome. This is the sole class-writer, so guarding here also neutralizes every
  // toggle (they still persist to localStorage, but it is never read into the class while embedded)
  // and overrides the system `prefers-color-scheme` preference.
  const effective = isEmbedded() ? 'light' : theme
  document.documentElement.classList.toggle('dark', effective === 'dark')
}

export function persistThemePreference(theme: ThemePreference) {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme)
  }

  applyThemePreference(theme)
}
