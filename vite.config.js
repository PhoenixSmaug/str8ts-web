import { defineConfig } from 'vite';

export default defineConfig({
  base: '/str8ts-web/',

  // Serve public/ as the static asset root so pkg/ is reachable at /pkg/
  publicDir: 'public',

  build: {
    outDir: 'dist',
    // Keep asset filenames deterministic (no content hash on the entry JS)
    // so the WASM relative paths remain stable.
    rollupOptions: {
      output: {
        entryFileNames: 'assets/[name].js',
        chunkFileNames: 'assets/[name].js',
        assetFileNames: 'assets/[name].[ext]',
      },
    },
  },
});
