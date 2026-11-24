import { createConnection } from 'node:net'

/**
 * Lists Redis keys for tests that pin the score keys the engine writes. Production scores live
 * under these keys, so a change to their shape orphans learned scores; asserting on the keys
 * themselves is the only way to catch that from outside the engine.
 *
 * Speaks just enough RESP over a raw socket to run SCAN, so the suite needs no Redis client.
 * REDIS_HOST / REDIS_PORT default to the docker-compose stack (localhost:6379).
 */

const HOST = process.env.REDIS_HOST || 'localhost'
const PORT = Number(process.env.REDIS_PORT || 6379)

function encode(args: string[]): string {
  return `*${args.length}\r\n${args.map((arg) => `$${Buffer.byteLength(arg)}\r\n${arg}\r\n`).join('')}`
}

type Reply = string | number | null | Reply[]

/** Parses one RESP reply from `buffer` at `offset`; `undefined` when it is not all there yet. */
function parse(buffer: Buffer, offset = 0): { value: Reply; next: number } | undefined {
  const lineEnd = buffer.indexOf('\r\n', offset)
  if (lineEnd < 0) return undefined
  const type = String.fromCharCode(buffer[offset])
  const line = buffer.toString('utf8', offset + 1, lineEnd)
  const next = lineEnd + 2
  switch (type) {
    case '+':
      return { value: line, next }
    case '-':
      throw new Error(`Redis error: ${line}`)
    case ':':
      return { value: Number(line), next }
    case '$': {
      const length = Number(line)
      if (length < 0) return { value: null, next }
      if (buffer.length < next + length + 2) return undefined
      return { value: buffer.toString('utf8', next, next + length), next: next + length + 2 }
    }
    case '*': {
      const count = Number(line)
      const items: Reply[] = []
      let cursor = next
      for (let i = 0; i < count; i++) {
        const item = parse(buffer, cursor)
        if (!item) return undefined
        items.push(item.value)
        cursor = item.next
      }
      return { value: items, next: cursor }
    }
    default:
      throw new Error(`Unexpected RESP type ${type}`)
  }
}

function command(args: string[]): Promise<Reply> {
  return new Promise((resolve, reject) => {
    const socket = createConnection({ host: HOST, port: PORT })
    let buffer = Buffer.alloc(0)
    socket.setTimeout(5_000, () => socket.destroy(new Error('Redis timed out')))
    socket.on('error', reject)
    socket.on('data', (chunk) => {
      buffer = Buffer.concat([buffer, chunk])
      try {
        const reply = parse(buffer)
        if (reply) {
          socket.end()
          resolve(reply.value)
        }
      } catch (error) {
        socket.destroy()
        reject(error)
      }
    })
    socket.write(encode(args))
  })
}

/** Every key matching a glob `pattern`, sorted. */
export async function redisKeys(pattern: string): Promise<string[]> {
  const keys: string[] = []
  let cursor = '0'
  do {
    const [next, batch] = (await command(['SCAN', cursor, 'MATCH', pattern, 'COUNT', '1000'])) as [
      string,
      string[],
    ]
    keys.push(...batch)
    cursor = next
  } while (cursor !== '0')
  return keys.sort()
}

/** `PFCOUNT` of the HyperLogLog at `key`: 0 when it doesn't exist. */
export async function redisPfcount(key: string): Promise<number> {
  return (await command(['PFCOUNT', key])) as number
}

/** Seconds until the key at `key` expires: -1 without an expiry, -2 when it doesn't exist. */
export async function redisTtl(key: string): Promise<number> {
  return (await command(['TTL', key])) as number
}

/** Length of the list at `key`: 0 when it doesn't exist. */
export async function redisListLength(key: string): Promise<number> {
  return (await command(['LLEN', key])) as number
}

/**
 * Refuses SET on keys starting with `prefix` (every other key still takes it) until `allowRedisSet`,
 * to exercise how the engine handles a write Redis rejects without failing unrelated requests.
 *
 * ACL key patterns only allow, so SET is withdrawn and granted back by a selector whose globs match
 * every key that does not start with `prefix`: one that diverges at each position, or is shorter.
 */
export async function refuseRedisSet(prefix: string): Promise<void> {
  if (/[*?[\]\\^-]/.test(prefix)) throw new Error(`prefix ${prefix} has glob characters`)
  const patterns: string[] = []
  for (let i = 0; i < prefix.length; i++) {
    patterns.push(`~${prefix.slice(0, i)}[^${prefix[i]}]*`)
    if (i > 0) patterns.push(`~${prefix.slice(0, i)}`)
  }
  await command(['ACL', 'SETUSER', 'default', '-set', `(+set ${patterns.join(' ')})`])
}

/** Undoes `refuseRedisSet`; harmless when nothing was refused. */
export async function allowRedisSet(): Promise<void> {
  await command(['ACL', 'SETUSER', 'default', '+set', 'clearselectors'])
}

/**
 * Distinct payments the A/B guardrail counted per arm in an experiment's current observation
 * period, keyed `<arm>_txns` and `<arm>_succ`. Exact for a handful of payments; from a few dozen,
 * HyperLogLog can read one or two low, so assert those with a margin.
 */
export async function abGuardrailCounts(merchantId: string, experimentId: string): Promise<Record<string, number>> {
  const counts: Record<string, number> = {}
  for (const key of await redisKeys(`ab_guardrail_{${merchantId}_${experimentId}}_*`)) {
    // The routed-payment sets aren't counts.
    if (key.endsWith('_routed')) continue
    // ab_guardrail_{m_e}_<arm>_<txns|succ>
    counts[key.split('}_')[1]] = await redisPfcount(key)
  }
  return counts
}

/** How many connections are subscribed to `channel`. */
export async function redisSubscribers(channel: string): Promise<number> {
  const [, count] = (await command(['PUBSUB', 'NUMSUB', channel])) as [string, number]
  return count
}

/**
 * Closes every pub/sub connection and refuses SUBSCRIBE until `restorePubSub`, so whatever is
 * published meanwhile reaches no instance: the gap a Redis failover leaves. Instances stay
 * connected for everything else.
 */
export async function cutPubSub(): Promise<void> {
  await command(['ACL', 'SETUSER', 'default', '-subscribe'])
  await command(['CLIENT', 'KILL', 'TYPE', 'pubsub'])
}

/** Allows SUBSCRIBE again after `cutPubSub`; harmless when it was never cut. */
export async function restorePubSub(): Promise<void> {
  await command(['ACL', 'SETUSER', 'default', '+subscribe'])
}
