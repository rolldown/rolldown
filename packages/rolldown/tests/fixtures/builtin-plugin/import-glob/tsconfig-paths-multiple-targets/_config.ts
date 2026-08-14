import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin, viteResolvePlugin } from 'rolldown/experimental';
import { expect, vi } from 'vitest';

// `@x/*` maps to multiple targets (`./a/*` and `./b/*`).
// Only the first (`./a/*`) is used for the glob, and a warning is emitted about the ignored targets.
const onMultipleTargetsWarn = vi.fn();

export default defineTest({
  config: {
    input: './src/main.ts',
    onLog(level, log) {
      if (level === 'warn' && log.message.includes('multiple targets')) {
        expect(log.message).toContain('@x/dir/**/*');
        onMultipleTargetsWarn();
      }
    },
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
    expect(onMultipleTargetsWarn).toHaveBeenCalledTimes(1);
    await import('./assert.mjs');
  },
});
