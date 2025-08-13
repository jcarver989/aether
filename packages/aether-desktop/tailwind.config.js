/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        'mono': ['SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', 'Source Code Pro', 'Menlo', 'Consolas', 'monospace'],
      },
      animation: {
        'pulse-subtle': 'pulse-subtle 2s ease-in-out infinite',
        'terminal-blink': 'terminal-blink 1s ease-in-out infinite',
        'scan-lines': 'scan-lines 2s linear infinite',
      },
      keyframes: {
        'pulse-subtle': {
          '0%, 100%': { 
            opacity: '1',
            transform: 'scale(1)' 
          },
          '50%': { 
            opacity: '0.95',
            transform: 'scale(1.002)' 
          },
        },
        'terminal-blink': {
          '0%, 50%': { opacity: '1' },
          '51%, 100%': { opacity: '0' },
        },
        'scan-lines': {
          '0%': { transform: 'translateY(-100%)' },
          '100%': { transform: 'translateY(100vh)' },
        }
      },
      boxShadow: {
        'retro': '0 0 15px hsl(240 100% 70% / 0.4)',
        'retro-inset': 'inset 0 0 15px hsl(240 100% 70% / 0.2)',
        'terminal-glow': '0 0 25px hsl(270 95% 75% / 0.15)',
      },
      borderWidth: {
        '3': '3px',
      }
    },
  },
  plugins: [
    require('@tailwindcss/typography'),
  ],
}