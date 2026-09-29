import { isEmbedded } from './embedMode'
import { postToDashboard } from './embedBridge'

const colorTokens = {
  primary: 'brand-600',
  background: 'page',
  surface: 'surface',
  text: 'text',
  mutedText: 'muted-text',
  border: 'border',
  link: 'link',
  linkHover: 'link-hover',
  primaryButtonBackground: 'button-primary',
  primaryButtonText: 'button-primary-text',
  primaryButtonHover: 'button-primary-hover',
  secondaryButtonBackground: 'button-secondary',
  secondaryButtonText: 'button-secondary-text',
  secondaryButtonHover: 'button-secondary-hover',
} as const

type ColorToken = keyof typeof colorTokens
export type EmbedThemeTokens = Partial<Record<ColorToken, string>> & {
  fontFamily?: string
  fontSize?: string
  headingFontSize?: string
  radius?: string
}

const fontFamilies = new Set([
  'Roboto, sans-serif',
  'Inter, sans-serif',
  'Arial, sans-serif',
  'Helvetica, Arial, sans-serif',
  'system-ui, sans-serif',
])
const sizeTokens = { fontSize: [12, 20], headingFontSize: [18, 36], radius: [0, 24] } as const
const appliedProperties = new Set<string>()
let stopThemeBridge: (() => void) | undefined

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

export function parseEmbedThemeTokens(value: unknown): EmbedThemeTokens | null {
  if (!isRecord(value) || Object.keys(value).length > 32) return null
  const tokens: EmbedThemeTokens = {}
  for (const key of Object.keys(colorTokens) as ColorToken[]) {
    const color = value[key]
    if (typeof color === 'string' && /^#[\da-f]{6}$/i.test(color)) tokens[key] = color
  }
  if (typeof value.fontFamily === 'string' && fontFamilies.has(value.fontFamily)) {
    tokens.fontFamily = value.fontFamily
  }
  for (const key of Object.keys(sizeTokens) as (keyof typeof sizeTokens)[]) {
    const size = value[key]
    if (typeof size !== 'string' || !/^\d{1,2}(\.\d{1,2})?px$/.test(size)) continue
    const [min, max] = sizeTokens[key]
    if (parseFloat(size) >= min && parseFloat(size) <= max) tokens[key] = size
  }
  return tokens
}

function rgb(hex: string): number[] {
  return [1, 3, 5].map((start) => parseInt(hex.slice(start, start + 2), 16))
}

function setProperty(name: string, value: string) {
  document.documentElement.style.setProperty(name, value)
  appliedProperties.add(name)
}

function clearProperties() {
  for (const name of appliedProperties) document.documentElement.style.removeProperty(name)
  appliedProperties.clear()
}

function applyTokens(tokens: EmbedThemeTokens) {
  clearProperties()
  for (const key of Object.keys(colorTokens) as ColorToken[]) {
    const value = tokens[key]
    if (value) setProperty(`--de-${colorTokens[key]}`, rgb(value).join(' '))
  }
  if (tokens.primary) {
    const base = rgb(tokens.primary)
    const shades = {
      50: 0.94,
      100: 0.88,
      200: 0.75,
      300: 0.55,
      400: 0.3,
      500: 0,
      600: 0,
      700: -0.2,
      800: -0.4,
      900: -0.6,
      950: -0.75,
    }
    for (const [shade, mix] of Object.entries(shades)) {
      const channels = base.map((channel) =>
        Math.round(mix >= 0 ? channel + (255 - channel) * mix : channel * (1 + mix)),
      )
      setProperty(`--de-brand-${shade}`, channels.join(' '))
    }
  }
  if (tokens.fontFamily) setProperty('--font-sans', tokens.fontFamily)
  if (tokens.fontSize) setProperty('--de-font-size', tokens.fontSize)
  if (tokens.headingFontSize) setProperty('--de-heading-size', tokens.headingFontSize)
  if (tokens.radius) setProperty('--de-radius', tokens.radius)
}

export function clearEmbeddedTheme() {
  stopThemeBridge?.()
  stopThemeBridge = undefined
  clearProperties()
  delete document.documentElement.dataset.themePending
}

export function initializeEmbeddedTheme() {
  if (!isEmbedded()) return
  clearEmbeddedTheme()
  const frameId = Array.from(crypto.getRandomValues(new Uint32Array(4)), (value) =>
    value.toString(16).padStart(8, '0'),
  ).join('')
  let revision = -1
  document.documentElement.dataset.embedded = 'true'
  document.documentElement.dataset.themePending = 'true'
  const reveal = () => {
    delete document.documentElement.dataset.themePending
  }
  const timeout = window.setTimeout(reveal, 1000)
  const ready = () => postToDashboard({ type: 'de:theme-ready', version: 1, frameId })
  const receive = (event: MessageEvent<unknown>) => {
    if (
      event.origin !== window.location.origin ||
      event.source !== window.parent ||
      !isRecord(event.data)
    )
      return
    const message = event.data
    if (message.version !== 1) return
    if (message.type === 'de:theme-request') {
      ready()
      return
    }
    const nextRevision = message.revision
    if (
      message.type !== 'de:theme-update' ||
      message.frameId !== frameId ||
      typeof nextRevision !== 'number' ||
      !Number.isSafeInteger(nextRevision) ||
      nextRevision < revision ||
      nextRevision < 0
    )
      return
    const tokens = parseEmbedThemeTokens(message.tokens)
    if (!tokens) return
    applyTokens(tokens)
    revision = nextRevision
    window.clearTimeout(timeout)
    reveal()
    postToDashboard({ type: 'de:theme-applied', version: 1, frameId, revision })
  }
  window.addEventListener('message', receive)
  stopThemeBridge = () => {
    window.clearTimeout(timeout)
    window.removeEventListener('message', receive)
  }
  ready()
}
