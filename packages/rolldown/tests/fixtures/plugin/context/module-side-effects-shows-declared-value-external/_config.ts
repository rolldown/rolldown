import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const ids = ['ext-none', 'ext-option-false', 'ext-hook-true', 'node:fs'];
const seen: Record<string, unknown> = {};

// An external shows the hook value, as in Rollup; else what `treeshake.moduleSideEffects`
// declares; else `false` for a Node.js built-in, which tree shaking drops by a Rolldown default,
// and `true` for other externals.
export default defineTest({
  config: {
    platform: 'node',
    external: ['ext-none', 'ext-option-false'],
    treeshake: {
      moduleSideEffects: (id) => (id === 'ext-option-false' ? false : undefined),
    },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source) {
          if (source === 'ext-hook-true') {
            return { id: source, external: true, moduleSideEffects: true };
          }
        },
        buildEnd() {
          for (const id of ids) seen[id] = this.getModuleInfo(id)!.moduleSideEffects;
        },
      },
    ],
  },
  afterTest(output) {
    expect(seen).toStrictEqual({
      'ext-none': true,
      'ext-option-false': false,
      'ext-hook-true': true,
      'node:fs': false,
    });
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain(`import "ext-none"`);
    expect(chunk.code).not.toContain('ext-option-false');
    expect(chunk.code).toContain(`import "ext-hook-true"`);
  },
});
