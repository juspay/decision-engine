import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  base: '/dashboard/',
  server: {
    proxy: {
      '/decide-gateway': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/routing': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/rule': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/merchant-account': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/config-sr-dimension': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/config': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/health': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/update-gateway-score': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/auth': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/api-key': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
      '/onboarding': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            console.log(`\n[PROXY] ${new Date().toISOString()}`)
            console.log(`Forwarding: ${req.method} ${req.url} -> http://localhost:8080${req.url}`)
          })
          proxy.on('proxyRes', (proxyRes, req) => {
            console.log(`[PROXY] Response: ${proxyRes.statusCode} ${proxyRes.statusMessage} for ${req.url}`)
          })
          proxy.on('error', (err, req) => {
            console.log(`\n[PROXY ERROR] ${new Date().toISOString()}`)
            console.log(`Error forwarding ${req.url}:`, err.message)
          })
        },
      },
    },
    fs: {
      strict: false,
    },
    historyApiFallback: {
      rewrites: [
        { from: /./, to: '/dashboard/index.html' }
      ]
    },
    host: true,
    port: 5173,
  },
  build: {
    outDir: 'dist',
  },
})
