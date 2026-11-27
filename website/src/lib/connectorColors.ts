/**
 * One colour per connector, shared by every page that draws connectors, so Stripe is the same
 * colour on Routing events, the Decision Explorer and the Simulator.
 *
 * The common connectors have a fixed colour each (Stripe and Adyen keep the ones the Simulator
 * already used). Any other connector gets a colour from its name, so it is the same on every load
 * without anything being stored. Red, orange and amber are left out: pages use them for warnings,
 * and a connector drawn in red reads as failing.
 */
export const CONNECTOR_COLORS: Readonly<Record<string, string>> = {
  stripe: '#8b5cf6',
  adyen: '#0abf53',
  checkout: '#0ea5e9',
  razorpay: '#2563eb',
  paypal: '#c026d3',
  braintree: '#0d9488',
  dlocal: '#65a30d',
  payu: '#4f46e5',
}

const FALLBACK_COLORS = ['#0284c7', '#059669', '#7c3aed', '#db2777', '#0f766e', '#4d7c0f']

function nameHash(name: string): number {
  let hash = 5381
  for (let i = 0; i < name.length; i += 1) hash = ((hash << 5) + hash + name.charCodeAt(i)) >>> 0
  return hash
}

/** The connector's fixed colour, or `undefined` when it has none. Case-insensitive. */
export function fixedConnectorColor(name: string): string | undefined {
  return CONNECTOR_COLORS[name.toLowerCase()]
}

/** The connector's colour on its own: the fixed one, else one picked from its name. */
export function connectorColor(name: string): string {
  return fixedConnectorColor(name) ?? FALLBACK_COLORS[nameHash(name.toLowerCase()) % FALLBACK_COLORS.length]
}

/**
 * Colours for the connectors on one page. Fixed colours never move. A connector without one
 * starts from its name's colour and, if a connector already on the page has that colour, takes
 * the next free fallback colour (in name order, so the result does not depend on list order).
 */
export function connectorColors(names: Iterable<string>): (name: string) => string {
  const unique = Array.from(new Set(names)).sort()
  const assigned = new Map<string, string>()
  const used = new Set<string>()
  for (const name of unique) {
    const fixed = fixedConnectorColor(name)
    if (fixed) {
      assigned.set(name, fixed)
      used.add(fixed)
    }
  }
  for (const name of unique) {
    if (assigned.has(name)) continue
    const start = nameHash(name.toLowerCase()) % FALLBACK_COLORS.length
    let color = FALLBACK_COLORS[start]
    for (let k = 0; k < FALLBACK_COLORS.length; k += 1) {
      const candidate = FALLBACK_COLORS[(start + k) % FALLBACK_COLORS.length]
      if (!used.has(candidate)) {
        color = candidate
        break
      }
    }
    assigned.set(name, color)
    used.add(color)
  }
  return (name) => assigned.get(name) ?? connectorColor(name)
}
