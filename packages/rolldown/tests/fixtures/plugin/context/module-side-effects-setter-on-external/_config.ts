import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// Without `experimental.importAttributes`, plugins see the raw id, so the write reaches both
// externals.
export default defineTest({
  config: {
    external: ['ext-pkg'],
    plugins: [
      {
        name: 'test-plugin',
        buildEnd() {
          const info = this.getModuleInfo('ext-pkg')!;
          info.moduleSideEffects = false;
          expect(info.moduleSideEffects).toBe(false);
        },
      },
    ],
  },
  afterTest(output) {
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).toContain('other');
    expect(chunk.code).not.toContain('ext-pkg');
  },
});
