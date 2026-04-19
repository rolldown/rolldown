import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'browser',
  dev: {
    port: 3637,
  },
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: {
        lazy: true,
      },
    },
  },
});
