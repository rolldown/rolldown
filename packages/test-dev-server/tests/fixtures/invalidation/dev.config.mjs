import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'node',
  build: {
    input: 'src/main.js',
    experimental: {
      devMode: {},
    },
    platform: 'node',
    treeshake: false,
  },
});
