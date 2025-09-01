import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  build: {
    input: 'src/main.js',
    experimental: {
      hmr: {
        new: true,
      },
      incrementalBuild: true,
      strictExecutionOrder: true,
    },
    treeshake: false,
  },
});
