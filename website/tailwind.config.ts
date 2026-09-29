import type { Config } from 'tailwindcss'

const token = (name: string, fallback: string) => `rgb(var(--de-${name}, ${fallback}) / <alpha-value>)`

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
          DEFAULT: token('brand-600', '12 105 238'),
          50: token('brand-50', '238 245 255'),
          100: token('brand-100', '217 234 255'),
          200: token('brand-200', '191 219 254'),
          300: token('brand-300', '147 197 253'),
          400: token('brand-400', '96 165 250'),
          500: token('brand-500', '59 130 246'),
          600: token('brand-600', '12 105 238'),
          700: token('brand-700', '9 84 190'),
          800: token('brand-800', '7 63 143'),
          900: token('brand-900', '6 45 102'),
          950: token('brand-950', '4 28 61'),
        },
      },
      backgroundColor: {
        page: token('page', '255 255 255'),
        white: token('surface', '255 255 255'),
        'button-primary': 'rgb(var(--de-button-primary, var(--de-brand-600, 12 105 238)) / <alpha-value>)',
        'button-primary-hover': 'rgb(var(--de-button-primary-hover, var(--de-brand-700, 9 84 190)) / <alpha-value>)',
        'button-secondary': token('button-secondary', '255 255 255'),
        'button-secondary-hover': token('button-secondary-hover', '248 250 252'),
      },
      textColor: {
        'button-primary': token('button-primary-text', '255 255 255'),
        'button-secondary': token('button-secondary-text', '51 65 85'),
        'button-secondary-hover': token('button-secondary-text', '15 23 42'),
        slate: {
          500: token('muted-text', '100 116 139'),
          600: token('muted-text', '71 85 105'),
          700: token('text', '51 65 85'),
          800: token('text', '30 41 59'),
          900: token('text', '15 23 42'),
          950: token('text', '2 6 23'),
        },
      },
      borderColor: {
        slate: {
          100: token('border', '241 245 249'),
          200: token('border', '226 232 240'),
          300: token('border', '203 213 225'),
        },
      },
      fontFamily: {
        // Both resolve through the variables index.css defines on `html`, so the whole app's
        // type changes in one place.
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
