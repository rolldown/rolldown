import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.ts',
    },
    platform: 'browser',
    experimental: {
      devMode: {},
    },
  },
});
