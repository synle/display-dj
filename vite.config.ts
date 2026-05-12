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
      // Explicit source globs (CLAUDE.md rule 41 — never `**/*` or `.`).
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.{test,spec}.{ts,tsx}',
        'src/test/**',
        'src/main.tsx',
        'src/**/*.d.ts',
        // Defensive exclusions (rule 41) — keep credentials, keys, and
        // binary assets out of coverage even if they ever appear under src/.
        '.env*',
        '**/secret*',
        '**/credential*',
        '**/*.pem',
        '**/*.key',
        '**/*.p12',
        'assets/binaries/**',
        'secrets/**',
      ],
      // Thresholds at the v7.0.3 baseline (Statements 75.59%, Branches
      // 73.35%, Functions 71.92%, Lines 77.56%) — held at the ≥60/60 floor
      // the user requested for this raise. A real regression past the
      // floor fails the build. Raise these as coverage improves; never
      // lower them.
      thresholds: {
        lines: 60,
        statements: 60,
        branches: 60,
        functions: 60,
      },
    },
  },
}));
