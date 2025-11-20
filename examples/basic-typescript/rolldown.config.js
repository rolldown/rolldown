import { defineConfig } from 'rolldown';

export default defineConfig({
  input: {
    entry: './index.ts',
  },
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  debug: {},
  plugins: [
    {
      name: 'test',
      renderChunk(code) {
        return code + '\n// test';
      },
    },
  ],
});
