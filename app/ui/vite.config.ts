import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: 'dist', target: 'es2022', chunkSizeWarningLimit: 600 },
  test: { environment: 'node', include: ['src/**/*.test.ts'] },
});
