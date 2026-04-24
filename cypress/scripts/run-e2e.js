#!/usr/bin/env node

const { spawn } = require('child_process')
const net = require('net')
const path = require('path')

const ROOT = path.resolve(__dirname, '../..')
const VALID_MODES = new Set(['source', 'docker', 'all'])
const DEFAULT_SPEC = 'cypress/e2e/**/*.cy.js'
const READINESS_TIMEOUT_MS = 180000
const READINESS_INTERVAL_MS = 2000
const KNOWN_COMPOSE_PROJECTS = [
  'decision-engine',
  'decision-engine-ui',
  'decision-engine-docs',
  'decision-engine-ccypress',
]
const EXPECTED_CLICKHOUSE_TABLES = [
  'analytics_api_events_queue',
  'analytics_domain_events_queue',
  'analytics_api_events',
  'analytics_domain_events',
  'analytics_payment_audit_summary_buckets',
  'analytics_payment_audit_lookup_summaries',
]

function parseArgs() {
  const args = process.argv.slice(2)
  const mode = args[0] || 'all'
  const keepAlive = args.includes('--keep-alive')

  if (!VALID_MODES.has(mode)) {
    throw new Error(`Unsupported E2E mode '${mode}'. Use source, docker, or all.`)
  }

  return { mode, keepAlive }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function prefixStream(stream, prefix, target) {
  if (!stream) return

  let buffer = ''
  stream.on('data', (chunk) => {
    buffer += chunk.toString()
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''
    lines.forEach((line) => target.write(`[${prefix}] ${line}\n`))
  })
  stream.on('end', () => {
    if (buffer) {
      target.write(`[${prefix}] ${buffer}\n`)
    }
  })
}

function spawnCommand(name, command, args, options = {}) {
  const child = spawn(command, args, {
    cwd: ROOT,
    env: { ...process.env, ...(options.env || {}) },
    shell: options.shell || false,
    detached: options.detached || false,
    stdio: options.stdio || ['ignore', 'pipe', 'pipe'],
  })

  prefixStream(child.stdout, name, process.stdout)
  prefixStream(child.stderr, name, process.stderr)

  return child
}

function runCommand(name, command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: ROOT,
      env: { ...process.env, ...(options.env || {}) },
      shell: options.shell || false,
      stdio: options.stdio || 'inherit',
    })

    child.on('exit', (code) => {
      if (code === 0) {
        resolve()
      } else {
        reject(new Error(`${name} failed with exit code ${code}`))
      }
    })
    child.on('error', reject)
  })
}

async function waitForHttpOk(label, url, options = {}) {
  const timeoutMs = options.timeoutMs || READINESS_TIMEOUT_MS
  const startedAt = Date.now()

  while (Date.now() - startedAt < timeoutMs) {
    try {
      const response = await fetch(url, {
        headers: options.headers,
      })
      if (response.ok) {
        return
      }
    } catch {
      // retry
    }

    if (options.process && options.process.exitCode !== null) {
      throw new Error(`${label} did not become ready because the source process exited early`)
    }

    await sleep(READINESS_INTERVAL_MS)
  }

  throw new Error(`${label} did not become ready within ${timeoutMs}ms`)
}

async function waitForTcp(label, host, port, options = {}) {
  const timeoutMs = options.timeoutMs || READINESS_TIMEOUT_MS
  const startedAt = Date.now()

  while (Date.now() - startedAt < timeoutMs) {
    const connected = await new Promise((resolve) => {
      const socket = net.createConnection({ host, port })
      const timeout = setTimeout(() => {
        socket.destroy()
        resolve(false)
      }, 1500)

      socket.on('connect', () => {
        clearTimeout(timeout)
        socket.end()
        resolve(true)
      })

      socket.on('error', () => {
        clearTimeout(timeout)
        resolve(false)
      })
    })

    if (connected) {
      return
    }

    if (options.process && options.process.exitCode !== null) {
      throw new Error(`${label} did not become ready because the source process exited early`)
    }

    await sleep(READINESS_INTERVAL_MS)
  }

  throw new Error(`${label} did not become ready within ${timeoutMs}ms`)
}

async function queryClickHouse(runtime, query) {
  const url = new URL(runtime.clickhouseHttpUrl)
  url.searchParams.set('database', runtime.clickhouseDatabase)
  url.searchParams.set('query', query)

  const auth = Buffer.from(`${runtime.clickhouseUser}:${runtime.clickhousePassword}`).toString('base64')
  const response = await fetch(url, {
    headers: {
      Authorization: `Basic ${auth}`,
    },
  })
  const body = await response.text()

  if (!response.ok) {
    throw new Error(`ClickHouse query failed (${response.status}): ${body}`)
  }

  return body
}

async function waitForClickHouseTables(runtime, processHandle) {
  const timeoutMs = READINESS_TIMEOUT_MS
  const startedAt = Date.now()

  while (Date.now() - startedAt < timeoutMs) {
    try {
      const rows = await queryClickHouse(
        runtime,
        `SELECT name FROM system.tables WHERE database = currentDatabase() AND name IN (${EXPECTED_CLICKHOUSE_TABLES.map((table) => `'${table}'`).join(', ')}) ORDER BY name FORMAT TSV`,
      )
      const found = new Set(
        rows
          .split('\n')
          .map((line) => line.trim())
          .filter(Boolean),
      )

      if (EXPECTED_CLICKHOUSE_TABLES.every((table) => found.has(table))) {
        return
      }
    } catch {
      // retry
    }

    if (processHandle && processHandle.exitCode !== null) {
      throw new Error('ClickHouse schema did not become ready because the source process exited early')
    }

    await sleep(READINESS_INTERVAL_MS)
  }

  throw new Error('ClickHouse schema did not become ready in time')
}

async function waitForRuntime(runtime, processHandle = null) {
  console.log(`\n[${runtime.mode}] Waiting for runtime readiness...`)

  await waitForTcp('Postgres', '127.0.0.1', 5432, { process: processHandle })
  await waitForTcp('Redis', '127.0.0.1', 6379, { process: processHandle })
  await waitForTcp('Kafka', '127.0.0.1', 9092, { process: processHandle })
  await waitForHttpOk('ClickHouse', `${runtime.clickhouseHttpUrl}/ping`, {
    process: processHandle,
    headers: {
      Authorization: `Basic ${Buffer.from(`${runtime.clickhouseUser}:${runtime.clickhousePassword}`).toString('base64')}`,
    },
  })
  await waitForClickHouseTables(runtime, processHandle)
  await waitForHttpOk('Decision Engine API', `${runtime.apiBaseUrl}/health`, { process: processHandle })
  await waitForHttpOk('Dashboard UI', `${runtime.uiBaseUrl}/`, { process: processHandle })
  await waitForHttpOk('Docs site', `${runtime.docsBaseUrl}/introduction`, { process: processHandle })
}

async function runCypress(runtime) {
  console.log(`\n[${runtime.mode}] Running Cypress suite...`)

  await runCommand(
    `cypress-${runtime.mode}`,
    'npx',
    ['cypress', 'run', '--spec', process.env.CYPRESS_E2E_SPEC || DEFAULT_SPEC],
    {
      env: {
        CYPRESS_RUNTIME_MODE: runtime.mode,
        CYPRESS_API_BASE_URL: runtime.apiBaseUrl,
        CYPRESS_UI_BASE_URL: runtime.uiBaseUrl,
        CYPRESS_DOCS_BASE_URL: runtime.docsBaseUrl,
        CYPRESS_CLICKHOUSE_HTTP_URL: runtime.clickhouseHttpUrl,
        CYPRESS_CLICKHOUSE_DATABASE: runtime.clickhouseDatabase,
        CYPRESS_CLICKHOUSE_USER: runtime.clickhouseUser,
        CYPRESS_CLICKHOUSE_PASSWORD: runtime.clickhousePassword,
      },
    },
  )
}

async function stopDockerServices(profile) {
  await runCommand(
    `docker-down-${profile}`,
    'bash',
    ['-lc', `COMPOSE_PROFILES= docker compose --profile ${profile} down --remove-orphans || true`],
  )
}

async function stopKnownComposeProjects() {
  for (const project of KNOWN_COMPOSE_PROJECTS) {
    await runCommand(
      `docker-down-${project}`,
      'bash',
      ['-lc', `docker compose -p ${project} down -v --remove-orphans || true`],
    )
  }
}

async function stopComposeInfra() {
  await runCommand(
    'source-infra-stop',
    'bash',
    ['-lc', 'COMPOSE_PROFILES= docker compose stop postgresql redis kafka kafka-init clickhouse || true'],
  )
}

async function killProcessGroup(child) {
  if (!child || child.exitCode !== null) {
    return
  }

  try {
    process.kill(-child.pid, 'SIGTERM')
  } catch {
    return
  }

  const startedAt = Date.now()
  while (child.exitCode === null && Date.now() - startedAt < 15000) {
    await sleep(500)
  }

  if (child.exitCode === null) {
    try {
      process.kill(-child.pid, 'SIGKILL')
    } catch {
      // ignore
    }
  }
}

async function runSourceMode(keepAlive) {
  const runtime = {
    mode: 'source',
    apiBaseUrl: 'http://localhost:8080',
    uiBaseUrl: 'http://localhost:5173',
    docsBaseUrl: 'http://localhost:3000',
    clickhouseHttpUrl: 'http://localhost:8123',
    clickhouseDatabase: 'default',
    clickhouseUser: 'decision_engine',
    clickhousePassword: 'decision_engine',
  }

  await stopKnownComposeProjects()
  const sourceProcess = spawnCommand('oneclick', 'bash', ['-lc', './oneclick.sh'], {
    detached: true,
    env: {
      ONECLICK_AUTO_CONFIRM: '1',
    },
  })

  const cleanup = async () => {
    await killProcessGroup(sourceProcess)
    if (!keepAlive) {
      await stopComposeInfra()
    }
  }

  try {
    await waitForRuntime(runtime, sourceProcess)
    await runCypress(runtime)
  } finally {
    await cleanup()
  }
}

async function runDockerMode(keepAlive) {
  const runtime = {
    mode: 'docker',
    apiBaseUrl: 'http://localhost:8080',
    uiBaseUrl: 'http://localhost:8081/dashboard',
    docsBaseUrl: 'http://localhost:8081',
    clickhouseHttpUrl: 'http://localhost:8123',
    clickhouseDatabase: 'default',
    clickhouseUser: 'decision_engine',
    clickhousePassword: 'decision_engine',
  }
  const composeProfile = 'dashboard-postgres-local'

  await stopKnownComposeProjects()
  await runCommand('dashboard-build', 'bash', ['-lc', 'npm --prefix website run build'])
  await runCommand(
    'docker-up',
    'bash',
    ['-lc', `COMPOSE_PROFILES= docker compose --profile ${composeProfile} up -d --build`],
  )

  const cleanup = async () => {
    if (!keepAlive) {
      await stopDockerServices(composeProfile)
    }
  }

  try {
    await waitForRuntime(runtime)
    await runCypress(runtime)
  } finally {
    await cleanup()
  }
}

async function main() {
  const { mode, keepAlive } = parseArgs()
  const modes = mode === 'all' ? ['source', 'docker'] : [mode]

  for (const selectedMode of modes) {
    if (selectedMode === 'source') {
      await runSourceMode(keepAlive)
    } else {
      await runDockerMode(keepAlive)
    }
  }
}

main().catch((error) => {
  console.error(`\n[E2E] ${error.message}`)
  process.exit(1)
})
