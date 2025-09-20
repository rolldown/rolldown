import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'node',
  build: {
    input: 'src/main.js',
    experimental: {
      hmr: {
        new: true,
      },
    },
    platform: 'node',
    treeshake: false,
  },
});
