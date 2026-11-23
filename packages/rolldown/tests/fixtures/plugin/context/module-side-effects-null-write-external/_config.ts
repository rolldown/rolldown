import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const seen: Record<string, unknown> = {};

// A `null` write to an external also sets its value back to "not decided".
export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: (id, external) =>
        external && id === 'ext-option-false' ? false : undefined,
    },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source) {
          if (source === 'ext-resolve-false') {
            return { id: source, external: true, moduleSideEffects: false };
          }
          if (source === 'ext-option-false') {
            return { id: source, external: true, moduleSideEffects: true };
          }
        },
        buildEnd() {
          for (const id of ['ext-resolve-false', 'ext-option-false']) {
            const info = this.getModuleInfo(id)!;
            info.moduleSideEffects = null;
            seen[id] = info.moduleSideEffects;
          }
        },
      },
    ],
  },
  afterTest(output) {
    expect(seen).toStrictEqual({ 'ext-resolve-false': true, 'ext-option-false': false });
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).toContain('ext-resolve-false');
    expect(chunk.code).not.toContain('ext-option-false');
  },
});
