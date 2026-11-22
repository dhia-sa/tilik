import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// Saat development (npm run dev), request /api diteruskan ke backend Rust.
// Build demo (GitHub Pages): DEMO_BASE=/tilik/ VITE_DEMO=1 npm run build
export default defineConfig({
  base: process.env.DEMO_BASE ?? '/',
  plugins: [react(), tailwindcss()],
  build: { chunkSizeWarningLimit: 1500 },
  server: { proxy: { '/api': 'http://127.0.0.1:8080' } },
})
