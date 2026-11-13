import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const websiteDir = path.resolve(scriptDir, '..')
const distDir = path.join(websiteDir, 'dist')
const rawBasePath = process.env.VITE_DASHBOARD_BASE_PATH || '/decision-engine/'

function normalizeBasePath(value) {
  const raw = `${value || ''}`.trim()
  if (!raw || raw === '/') return '/'
  return `/${raw.replace(/^\/+|\/+$/g, '')}/`
}

const basePath = normalizeBasePath(rawBasePath)
if (basePath === '/') {
  process.exit(0)
}

if (!existsSync(distDir)) {
  throw new Error(`Expected Vite output at ${distDir}`)
}

const prefixParts = basePath.replace(/^\/+|\/+$/g, '').split('/').filter(Boolean)
const targetDir = path.join(distDir, ...prefixParts)
if (!targetDir.startsWith(`${distDir}${path.sep}`)) {
  throw new Error(`Refusing to mirror dist outside ${distDir}`)
}

rmSync(targetDir, { recursive: true, force: true })
mkdirSync(targetDir, { recursive: true })

for (const entry of readdirSync(distDir)) {
  if (entry === prefixParts[0]) continue
  cpSync(path.join(distDir, entry), path.join(targetDir, entry), {
    recursive: true,
    dereference: true,
  })
}

console.log(`Mirrored dashboard assets under dist/${prefixParts.join('/')}/`)
