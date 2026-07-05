import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: { main: 'main.js' },
    platform: 'browser',
    treeshake: false,
    experimental: { devMode: {} },
  },
});
