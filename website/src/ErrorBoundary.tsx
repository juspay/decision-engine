import { Component, ReactNode } from 'react'

interface Props { children: ReactNode }
interface State { error: Error | null; errorInfo: React.ErrorInfo | null }

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, errorInfo: null }
  
  static getDerivedStateFromError(error: Error): State { 
    return { error, errorInfo: null } 
  }
  
  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.log('\n' + '!'.repeat(80))
    console.log('[ERROR BOUNDARY] Component Error Caught')
    console.log(`Timestamp: ${new Date().toISOString()}`)
    console.log('Error Message:', error.message)
    console.log('Error Stack:', error.stack)
    console.log('Component Stack:', errorInfo.componentStack)
    console.log('!'.repeat(80) + '\n')
    this.setState({ errorInfo })
  }
  
  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 32, fontFamily: 'monospace', color: 'red' }}>
          <h2>Dashboard Error</h2>
          <pre>{this.state.error.message}</pre>
          <pre>{this.state.error.stack}</pre>
          {this.state.errorInfo && (
            <pre style={{ marginTop: 16, color: 'darkred' }}>
              Component Stack:{this.state.errorInfo.componentStack}
            </pre>
          )}
        </div>
      )
    }
    return this.props.children
  }
}
