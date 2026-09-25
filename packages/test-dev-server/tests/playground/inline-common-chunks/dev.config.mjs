import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    experimental: {
      // Non-lazy: the routes compile into the initial build, so the page loads the route
      // chunks that carry the inlined common chunk. With lazy compilation the page would get
      // the routes as factories from `/@vite/lazy` and never load those chunks.
      // `inline-common-chunks-lazy` is the lazy variant.
      devMode: { lazy: false },
    },
    output: {
      codeSplitting: {
        experimentalInlineCommonChunks: { maxSize: 4096 },
      },
    },
  },
});
