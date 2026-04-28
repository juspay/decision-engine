import React from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import App from './App'
import { ErrorBoundary } from './ErrorBoundary'
import './index.css'

const routerBaseName = import.meta.env.BASE_URL.endsWith('/')
  ? import.meta.env.BASE_URL.slice(0, -1)
  : import.meta.env.BASE_URL

if (import.meta.env.DEV && window.location.hostname === '127.0.0.1') {
  const nextUrl = new URL(window.location.href)
  nextUrl.hostname = 'localhost'
  window.location.replace(nextUrl.toString())
} else {
  console.log('\n' + '='.repeat(80))
  console.log('[APP STARTUP] Dashboard initializing...')
  console.log(`Timestamp: ${new Date().toISOString()}`)
  console.log(`Environment: ${(import.meta as any).env?.MODE ?? 'production'}`)
  console.log(`Base URL: ${import.meta.env.BASE_URL}`)
  console.log('='.repeat(80) + '\n')

  window.onerror = (message, source, lineno, colno, error) => {
    console.log('\n' + '!'.repeat(80))
    console.log('[WINDOW ERROR]')
    console.log('Message:', message)
    console.log('Source:', source)
    console.log('Line:', lineno, 'Column:', colno)
    if (error) {
      console.log('Error:', error.message)
      console.log('Stack:', error.stack)
    }
    console.log('!'.repeat(80) + '\n')
  }

  window.onunhandledrejection = (event) => {
    console.log('\n' + '!'.repeat(80))
    console.log('[UNHANDLED PROMISE REJECTION]')
    console.log('Reason:', event.reason)
    if (event.reason instanceof Error) {
      console.log('Stack:', event.reason.stack)
    }
    console.log('!'.repeat(80) + '\n')
  }

  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <ErrorBoundary>
        <BrowserRouter basename={routerBaseName}>
          <App />
        </BrowserRouter>
      </ErrorBoundary>
    </React.StrictMode>
  )
}
