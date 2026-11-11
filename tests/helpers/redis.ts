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
