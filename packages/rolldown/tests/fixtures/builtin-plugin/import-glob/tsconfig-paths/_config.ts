import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin, viteResolvePlugin } from 'rolldown/experimental';

// `import.meta.glob('@/dir/**/*')` must resolve the `@/*` -> `./src/*` tsconfig paths mapping
export default defineTest({
  config: {
    input: './src/main.ts',
    plugins: [
      viteResolvePlugin({
        resolveOptions: {
          isBuild: true,
          isProduction: true,
          asSrc: false,
          preferRelative: false,
          root: import.meta.dirname,
          scan: false,
          mainFields: ['main'],
          conditions: [],
          externalConditions: [],
          extensions: ['.js'],
          tryIndex: false,
          preserveSymlinks: false,
          tsconfigPaths: true,
        },
        environmentConsumer: 'client',
        environmentName: 'test',
        builtins: [],
        external: [],
        noExternal: [],
        dedupe: [],
        legacyInconsistentCjsInterop: false,
        resolveSubpathImports() {
          throw new Error('Not implemented');
        },
      }),
      viteImportGlobPlugin({ root: import.meta.dirname }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
