import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  build: {
    input: 'src/main.js',
    experimental: {
      devMode: {
        new: true,
      },
      incrementalBuild: true,
      strictExecutionOrder: true,
    },
    treeshake: false,
  },
});
