import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig(({ command }) => {
  const isDevServer = command === 'serve'
  const publicBaseUrl = isDevServer ? '/' : '/decision-engine/'
  const backendTarget = 'http://localhost:8080'
  const apiProxyPrefix = '/decision-engine-api'
  const hostedApiProxyPrefix = '/decision-engine/api'

  const createApiProxy = (rewritePrefix?: string) => ({
    target: backendTarget,
    changeOrigin: true,
    rewrite: rewritePrefix ? (path) => path.replace(new RegExp(`^${rewritePrefix}`), '') : undefined,
    configure: (proxy) => {
      proxy.on('proxyReq', (_proxyReq, req) => {
        console.log(`\n[PROXY] ${new Date().toISOString()}`)
        console.log(`Forwarding: ${req.method} ${req.url} -> ${backendTarget}${req.url}`)
      })
      proxy.on('proxyRes', (proxyRes, req) => {
        console.log(
          `[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`
        )
      })
      proxy.on('error', (err, req) => {
        console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
        console.log(`Error forwarding ${req.url}:`, err.message)
      })
    },
  })

  return {
    plugins: [react()],
    base: publicBaseUrl,
    server: {
      proxy: {
        '^/decision-engine-api(?:/.*)?$': createApiProxy(apiProxyPrefix),
        '^/decision-engine/api(?:/.*)?$': createApiProxy(hostedApiProxyPrefix),
        '/decide-gateway': createApiProxy(),
        '/decision_gateway': createApiProxy(),
        '/merchant-account': createApiProxy(),
        '/config-sr-dimension': createApiProxy(),
        '^/config(?:/.*)?$': createApiProxy(),
        '/health': createApiProxy(),
        '/update-gateway-score': createApiProxy(),
        '/update-score': createApiProxy(),
        '^/rule(?:/.*)?$': createApiProxy(),
        '^/routing/(create|activate|evaluate|list(?:/.*)?|hybrid)$': createApiProxy(),
        '^/analytics/(overview|gateway-scores|decisions|routing-stats|log-summaries|preview-trace|payment-audit)(?:\\?.*)?$':
          createApiProxy(),
        '^/onboarding(?:/.*)?$': createApiProxy(),
        '^/auth(?:/.*)?$': createApiProxy(),
        '^/api-key(?:/.*)?$': createApiProxy(),
        '^/merchant(?:/.*)?$': createApiProxy(),
      },
      fs: {
        strict: false,
      },
      host: true,
      port: 5173,
    },
    build: {
      outDir: 'dist',
    },
    preview: {
      proxy: {
        '^/decision-engine-api(?:/.*)?$': createApiProxy(apiProxyPrefix),
        '/decide-gateway': createApiProxy(),
        '/decision_gateway': createApiProxy(),
        '/merchant-account': createApiProxy(),
        '/config-sr-dimension': createApiProxy(),
        '^/config(?:/.*)?$': createApiProxy(),
        '/health': createApiProxy(),
        '/health/ready': createApiProxy(),
        '/update-gateway-score': createApiProxy(),
        '/update-score': createApiProxy(),
        '^/rule(?:/.*)?$': createApiProxy(),
        '^/routing/(create|activate|evaluate|list(?:/.*)?|hybrid)$': createApiProxy(),
        '^/analytics/(overview|gateway-scores|decisions|routing-stats|log-summaries|preview-trace|payment-audit)(?:\\?.*)?$':
          createApiProxy(),
        '^/onboarding(?:/.*)?$': createApiProxy(),
        '^/auth(?:/.*)?$': createApiProxy(),
        '^/api-key(?:/.*)?$': createApiProxy(),
        '^/merchant(?:/.*)?$': createApiProxy(),
      },
    },
  }
})
