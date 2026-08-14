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
      // proxy). Lazy dynamic-import HMR is a separate follow-up.
      devMode: { lazy: false },
    },
  },
});
