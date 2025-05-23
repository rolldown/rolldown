import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  build: {
    input: 'src/main.js',
    experimental: {
      hmr: true,
    },
    platform: 'node',
    treeshake: false,
  },
});
