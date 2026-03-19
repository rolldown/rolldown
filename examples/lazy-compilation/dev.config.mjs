import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  build: {
    input: './src/entry-a.js',
    output: {
      strictExecutionOrder: true,
    },
    experimental: {
      devMode: {
        lazy: true,
      },
      incrementalBuild: true,
    },
    treeshake: false,
  },
});
