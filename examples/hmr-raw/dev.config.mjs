import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  build: {
    input: 'src/main.js',
    output: {
      strictExecutionOrder: true,
    },
    experimental: {
      devMode: {
        new: true,
      },
      incrementalBuild: true,
    },
    treeshake: false,
  },
});
