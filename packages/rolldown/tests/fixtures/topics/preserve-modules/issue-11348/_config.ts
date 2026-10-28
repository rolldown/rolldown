import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { getOutputChunk } from '../../../../src/utils';

export default defineTest({
  config: {
    output: {
      preserveModules: true,
    },
  },
  afterTest: (output) => {
    const chunks = getOutputChunk(output);
    const a = chunks.find((chunk) => chunk.fileName === 'a.js')!;
    const b = chunks.find((chunk) => chunk.fileName === 'b.js')!;
    // ESM evaluates b.js first, so b.js has no eager wrapper call (`export default require_b();`).
    expect(b.code).not.toContain('export default');
    expect(b.exports).toStrictEqual(['require_b']);
    expect(a.code).toContain('export default require_a();');
    expect(a.exports).toContain('default');
  },
});
