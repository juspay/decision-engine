import React from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import App from './App'
import { ErrorBoundary } from './ErrorBoundary'
import './index.css'

console.log('\n' + '='.repeat(80))
console.log('[APP STARTUP] Dashboard initializing...')
console.log(`Timestamp: ${new Date().toISOString()}`)
console.log(`Environment: ${import.meta.env.MODE}`)
console.log(`Base URL: /dashboard`)
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
      <BrowserRouter basename="/dashboard">
        <App />
      </BrowserRouter>
    </ErrorBoundary>
  </React.StrictMode>
)
