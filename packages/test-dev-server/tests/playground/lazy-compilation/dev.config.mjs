import path from 'node:path';
import url from 'node:url';
import { defineDevConfig } from '@rolldown/test-dev-server';
import { viteAliasPlugin } from 'rolldown/experimental';

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));

// One shared config for all lazy-compilation scenarios. Each scenario lives
// in its own folder and is imported by `main.js`. A lazy chunk compiles only
// when its dynamic import runs, so the scenarios don't warm each other up —
// every spec still gets a fresh first fetch, as if it had its own server.
//
// The config is the union of what the scenarios need:
// - `viteAliasPlugin` maps `@lazy` for aliased-import (vitejs/vite#22454);
//   harmless for the others.
// - `moduleTypes` makes emitted-asset's `.png` import emit a real hashed asset
//   (vitejs/vite#22596); the other scenarios import no assets, so it is a no-op
//   for them.
export default defineDevConfig({
  platform: 'browser',
  build: {
    input: { main: 'main.js' },
    platform: 'browser',
    treeshake: false,
    moduleTypes: {
      '.png': 'asset',
    },
    experimental: {
      devMode: { lazy: true },
      // `new-url` scenario: emit `new URL('./x', import.meta.url)` targets as
      // hashed assets (vitejs/vite#22596). No-op for the other scenarios.
      resolveNewUrlToAsset: true,
    },
    plugins: [
      viteAliasPlugin({
        entries: [
          { find: '@lazy', replacement: path.resolve(__dirname, 'aliased-import/lazy.js') },
        ],
      }),
    ],
  },
});
