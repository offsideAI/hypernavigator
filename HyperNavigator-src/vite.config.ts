import { resolve } from 'node:path';
import { defineConfig } from 'vite';

// Tauri expects a fixed dev port (tauri.conf.json build.devUrl).
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: 'localhost',
    watch: { ignored: ['**/src-tauri/**'] },
  },
  build: {
    target: 'safari16',
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, 'index.html'),
        placeholder: resolve(import.meta.dirname, 'placeholder.html'),
      },
    },
  },
});
