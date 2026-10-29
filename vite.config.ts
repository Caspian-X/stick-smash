import { defineConfig } from 'vitest/config';

export default defineConfig({
  base: './',
  server: { open: true },
  build: { chunkSizeWarningLimit: 2000 },
  test: { include: ['tests/**/*.test.ts'] },
});
