import { defineDevConfig } from '@rolldown/test-dev-server';

// No plugin here. Vite installs the native `builtin:vite-import-glob` plugin for the bundled
// environment itself (`applyToEnvironment` in vite's `plugins/importMetaGlob.ts`).
export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: { hotUpdate: true },
    },
  },
});
