import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        brand: {
          DEFAULT: 'var(--color-brand-600, #0c69ee)',
          50: 'var(--color-brand-50, #eef5ff)',
          100: 'var(--color-brand-100, #d9eaff)',
          200: 'var(--color-brand-200, #bfdbfe)',
          300: 'var(--color-brand-300, #93c5fd)',
          400: 'var(--color-brand-400, #60a5fa)',
          500: 'var(--color-brand-500, #3b82f6)',
          600: 'var(--color-brand-600, #0c69ee)',
          700: 'var(--color-brand-700, #0954be)',
          800: 'var(--color-brand-800, #073f8f)',
          900: 'var(--color-brand-900, #062d66)',
          950: 'var(--color-brand-950, #041c3d)',
        },
      },
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)'],
      },
      letterSpacing: {
        tightest: '-0.03em',
      },
      boxShadow: {
        'glass-light': '0 10px 40px -10px rgba(0,0,0,0.08), 0 0 0 1px rgba(0,0,0,0.05)',
        'glass-dark': '0 20px 40px rgba(0,0,0,0.4), inset 0 1px 0 rgba(255,255,255,0.05), inset 0 0 0 1px rgba(255,255,255,0.02)',
      },
      keyframes: {
        progress: {
          '0%': { width: '0%' },
          '100%': { width: '100%' },
        },
      },
      animation: {
        progress: 'progress 2.5s linear forwards',
      },
    },
  },
  plugins: [],
} satisfies Config
