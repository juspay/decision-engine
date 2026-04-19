import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig(({ command }) => {
  const isDevServer = command === 'serve'
  const publicBaseUrl = isDevServer ? '/' : '/dashboard/'
  const backendTarget = 'http://localhost:8080'

  const createApiProxy = () => ({
    target: backendTarget,
    changeOrigin: true,
    configure: (proxy) => {
      proxy.on('proxyReq', (_proxyReq, req) => {
        console.log(`\n[PROXY] ${new Date().toISOString()}`)
        console.log(`Forwarding: ${req.method} ${req.url} -> ${backendTarget}${req.url}`)
      })
      proxy.on('proxyRes', (proxyRes, req) => {
        console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
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
        '/decide-gateway': createApiProxy(),
        '/merchant-account': createApiProxy(),
        '/config-sr-dimension': createApiProxy(),
        '^/config(?:/.*)?$': createApiProxy(),
        '/health': createApiProxy(),
        '/update-gateway-score': createApiProxy(),
        '^/rule/(get|create|update|delete)$': createApiProxy(),
        '^/routing/(create|activate|evaluate|list(?:/.*)?)$': createApiProxy(),
        '^/analytics/(overview|routing-stats|preview-trace|payment-audit)(?:\\?.*)?$': createApiProxy(),
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
  }
})
