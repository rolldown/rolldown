import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      // Non-lazy: dynamic import() targets compile into the initial build (no lazy
      // proxy). `hmr-lazy-dynamic-import-self-accept` is the lazy variant.
      devMode: { lazy: false },
    },
  },
});
