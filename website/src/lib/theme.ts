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

  const effective = isEmbedded() ? 'light' : theme
  document.documentElement.classList.toggle('dark', effective === 'dark')
}

export function persistThemePreference(theme: ThemePreference) {
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme)
  }

  applyThemePreference(theme)
}

export function applyThemeTokens(tokens: Record<string, string>) {
  if (typeof document === 'undefined') return
  
  const root = document.documentElement
  
  // Allowlist of allowed CSS variable prefixes based on the issue scope
  const allowedPrefixes = ['--color-brand', '--color-surface', '--color-text', '--radius', '--font']
  
  for (const [key, value] of Object.entries(tokens)) {
    if (!key.startsWith('--') || typeof value !== 'string') continue
    
    const isAllowed = allowedPrefixes.some(prefix => key.startsWith(prefix))
    if (!isAllowed) continue

    // Validate and sanitize the value
    // Reject urls, css functions like calc, expressions, and semicolons
    if (/url\(|calc\(|expression\(|javascript:|;/i.test(value)) continue
    
    const safeValue = value.trim()
    
    // Only apply if length is reasonable to prevent arbitrary injection
    if (safeValue.length > 0 && safeValue.length < 50) {
      root.style.setProperty(key, safeValue)
    }
  }
}
