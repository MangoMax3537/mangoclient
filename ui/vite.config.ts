import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Wallpapers the user drops here are bundled (the folder may be empty or missing).
const backgrounds = fileURLToPath(new URL('../assets/backgrounds', import.meta.url));

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  resolve: { alias: { '@backgrounds': backgrounds } },
  server: { port: 5173, strictPort: false, fs: { allow: ['.', fileURLToPath(new URL('../assets', import.meta.url))] } },
  build: { outDir: 'dist', target: 'es2022', chunkSizeWarningLimit: 600 },
  test: {
    // `*.dom.test.ts` mount real components in jsdom (client build of Svelte, `browser` condition); everything else runs in node
    projects: [
      { extends: true, test: { name: 'unit', environment: 'node', include: ['src/**/*.test.ts'], exclude: ['src/**/*.dom.test.ts', 'node_modules/**'] } },
      { extends: true, resolve: { conditions: ['browser'] }, test: { name: 'dom', environment: 'jsdom', include: ['src/**/*.dom.test.ts'] } },
    ],
  },
});
