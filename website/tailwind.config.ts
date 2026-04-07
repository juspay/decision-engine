import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        brand: {
          DEFAULT: '#1668E3',
          50: 'rgba(22,104,227,0.08)',
          100: 'rgba(22,104,227,0.15)',
          500: '#1668E3',
          600: '#1254c4',
          700: '#0f44a8',
        },
        // Dark-first gray scale: 50=darkest surface, 900=brightest text
        gray: {
          50:  '#0e0e14',   // card/table header bg
          100: '#13131a',   // slightly elevated surface
          200: '#1e1e28',   // border color
          300: '#2c2c3a',   // strong border / input border
          400: '#72728a',   // muted/placeholder text — 4.5:1 on bg
          500: '#9898b0',   // secondary text — 6:1 on bg
          600: '#b4b4c8',   // body text / labels
          700: '#ccccdc',   // sub-headings
          800: '#dedeed',   // secondary headings
          900: '#f0f0fa',   // primary text
        },
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', '-apple-system', 'sans-serif'],
        mono: ['JetBrains Mono', 'Menlo', 'Monaco', 'monospace'],
      },
      letterSpacing: {
        tightest: '-0.03em',
      },
    },
  },
  plugins: [],
} satisfies Config
