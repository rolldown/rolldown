import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Rollup's `getResolvedIdWithDefaults` fills `moduleSideEffects` from
// `treeshake.moduleSideEffects` when no hook sets one. The default reports `true`
// for an external (`test/function/samples/plugin-module-information`). A hook
// value still wins. Platform stays neutral so `path` is not a pure Node builtin.
export default defineTest({
  config: {
    input: 'main.js',
    external: ['path'],
    plugins: [
      {
        name: 'report-external-module-side-effects',
        resolveId(source) {
          if (source === 'pure-external') {
            return { id: source, external: true, moduleSideEffects: false };
          }
        },
        async buildEnd() {
          const pathInfo = this.getModuleInfo('path');
          const pathResolved = await this.resolve('path');
          expect(pathInfo?.moduleSideEffects, 'getModuleInfo path').toBe(true);
          expect(pathResolved?.moduleSideEffects, 'this.resolve path').toBe(true);
          expect(pathResolved?.external, 'this.resolve path external').toBe(true);

          const pureInfo = this.getModuleInfo('pure-external');
          // `this.resolve` skips the calling plugin unless `skipSelf` is false, and
          // nothing else marks this id external.
          const pureResolved = await this.resolve('pure-external', undefined, {
            skipSelf: false,
          });
          expect(pureInfo?.moduleSideEffects, 'getModuleInfo pure-external').toBe(false);
          expect(pureResolved?.external, 'this.resolve pure-external external').toBe(true);
          expect(pureResolved?.moduleSideEffects, 'this.resolve pure-external').toBe(false);
        },
      },
    ],
  },
  afterTest(output) {
    const code = output.output
      .map((chunk) => (chunk.type === 'chunk' ? chunk.code : ''))
      .join('\n');
    // No hook value: the external stays included (`NoTreeshake`) and reports `true`.
    expect(code).toContain('"path"');
    // A hook `false` is still side-effect free, so the import is dropped.
    expect(code).not.toContain('pure-external');
  },
});
