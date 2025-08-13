/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        'mono': ['JetBrains Mono', 'Fira Code', 'Cascadia Code', 'SF Mono', 'Monaco', 'Roboto Mono', 'monospace'],
      },
      animation: {
        'pulse-glow': 'pulse-glow 3s ease-in-out infinite',
        'cursor-blink': 'cursor-blink 1.2s ease-in-out infinite',
        'hologram-scan': 'hologram-scan 4s linear infinite',
        'data-stream': 'data-stream 8s linear infinite',
        'shimmer': 'shimmer 2s ease-in-out infinite',
      },
      keyframes: {
        'pulse-glow': {
          '0%, 100%': { 
            opacity: '1',
            transform: 'scale(1)',
            boxShadow: '0 0 20px hsl(var(--primary) / 0.3)' 
          },
          '50%': { 
            opacity: '0.9',
            transform: 'scale(1.01)',
            boxShadow: '0 0 30px hsl(var(--primary) / 0.5)' 
          },
        },
        'cursor-blink': {
          '0%, 45%': { opacity: '1' },
          '50%, 100%': { opacity: '0.2' },
        },
        'hologram-scan': {
          '0%': { transform: 'translateY(-200%)' },
          '100%': { transform: 'translateY(calc(100vh + 200%))' },
        },
        'data-stream': {
          '0%': { transform: 'translateX(-100%)' },
          '100%': { transform: 'translateX(100%)' },
        },
        'shimmer': {
          '0%': { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        }
      },
      boxShadow: {
        'neon': '0 0 20px hsl(var(--primary) / 0.5), 0 0 40px hsl(var(--primary) / 0.3)',
        'neon-subtle': '0 0 10px hsl(var(--primary) / 0.3)',
        'hologram': 'inset 0 0 20px hsl(var(--primary) / 0.15), 0 0 30px hsl(var(--accent) / 0.1)',
        'interface': '0 4px 20px hsl(var(--background) / 0.8), 0 0 40px hsl(var(--primary) / 0.1)',
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