import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // A full 50–950 ramp: `dark:` variants across the app reach for brand-300/400/800/900, and
        // a missing key makes Tailwind emit nothing at all, so the light utility beside it survives
        // into dark mode. 50–100 and 500–700 keep the values the palette already shipped.
        brand: {
          DEFAULT: '#0c69ee',
          50: '#eef5ff',
          100: '#d9eaff',
          200: '#bfdbfe',
          300: '#93c5fd',
          400: '#60a5fa',
          500: '#3b82f6',
          600: '#0c69ee',
          700: '#0954be',
          800: '#073f8f',
          900: '#062d66',
          950: '#041c3d',
        },
      },
      fontFamily: {
        sans: ['Outfit', 'Inter', 'system-ui', '-apple-system', 'sans-serif'],
        mono: ['JetBrains Mono', 'Menlo', 'Monaco', 'monospace'],
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
