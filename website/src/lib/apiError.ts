/** Extract the human-readable message from an `API error <status>: <json>` error. */
export function getApiErrorMessage(err: unknown): string {
  const msg = err instanceof Error ? err.message : 'Something went wrong'
  const match = msg.match(/API error \d+: (.+)/)

  if (!match) return msg

  try {
    const parsed = JSON.parse(match[1])
    return parsed.message ?? msg
  } catch {
    return match[1]
  }
}
