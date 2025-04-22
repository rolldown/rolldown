import { defineConfig } from 'rolldown';

export default defineConfig({
  input: {
    entry: './index.ts',
  },
  plugins: [
    // {
    //   name: 'test:resolve',
    //   resolveId(id) {
    //     if (id.includes('hello.ts')) {
    //       return { id: 'test', external: true };
    //     }
    //     return null;
    //   },
    // },
  ],
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  debug: {},
});
