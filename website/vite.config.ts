import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  base: '/dashboard/',
  server: {
    proxy: {
      '/decide-gateway': 'http://localhost:8080',
      '/routing': 'http://localhost:8080',
      '/rule': 'http://localhost:8080',
      '/merchant-account': 'http://localhost:8080',
      '/config-sr-dimension': 'http://localhost:8080',
      '/health': 'http://localhost:8080',
    },
  },
  build: {
    outDir: 'dist',
  },
})
