// Build-time deployment configuration for the dashboard.

const APP_ENV = (import.meta.env.VITE_APP_ENV ?? 'development').trim().toLowerCase()

export const isProduction = APP_ENV === 'production'

export const signupEnabled = !isProduction

export const simulatorEnabled = !isProduction

export const sampleReportEnabled = !isProduction
