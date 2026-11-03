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
      // Lazy: dynamic import() targets compile on demand behind a `?rolldown-lazy=1` proxy.
      devMode: { lazy: true },
    },
  },
});
