import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  build: {
    input: './src/entry-a.js',
    experimental: {
      devMode: {
        lazy: true,
      },
      incrementalBuild: true,
      strictExecutionOrder: true,
    },
    treeshake: false,
  },
});
