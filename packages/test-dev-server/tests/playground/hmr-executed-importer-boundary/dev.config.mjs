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
      // Non-lazy: both parents compile into the initial build (no lazy proxy), so the
      // test exercises per-tab execution state, not lazy fetch state.
      devMode: { lazy: false },
    },
  },
});
