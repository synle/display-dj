import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'text-summary', 'json', 'html'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.{test,spec}.{ts,tsx}', 'src/test/**', 'src/main.tsx', 'src/**/*.d.ts'],
      // Thresholds pinned to the v7.0.2 baseline (Statements 36.93%,
      // Branches 25.32%, Functions 33.33%, Lines 37.8%) minus a ~1pt
      // safety margin against coincidental flakes. A real regression
      // past the floor fails the build. Raise these as coverage
      // improves; never lower them.
      thresholds: {
        lines: 36,
        statements: 35,
        branches: 24,
        functions: 32,
      },
    },
  },
}));
