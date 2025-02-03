import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [svelte()],

  // Vite options tailored for Tauri development
  root: resolve(__dirname),
  publicDir: resolve(__dirname, 'public'),
  base: './',

  // This ensures compatibility with Tauri
  server: {
    port: 1420,
    strictPort: true,
  },

  // Ensure the build output goes to the correct location
  build: {
    outDir: 'dist',
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    emptyOutDir: true,
  },

  // Ensure proper resolution of file paths
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
});