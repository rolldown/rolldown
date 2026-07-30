import { defineDevConfig } from '@rolldown/test-dev-server';

// No plugin configuration on purpose: on the browser platform Vite installs the native
// `builtin:vite-import-glob` plugin for the bundled environment itself (`importGlobPlugin`'s
// `applyToEnvironment` in vite's `plugins/importMetaGlob.ts`), so this playground exercises the real
// Vite -> rolldown path instead of a hand-wired one.
export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: {},
    },
  },
});
