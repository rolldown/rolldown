import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    treeshake: {
      annotations: false,
    },
  },
  afterTest: (output) => {
    const code = output.output
      .filter((chunk): chunk is OutputChunk => chunk.type === 'chunk')
      .map((chunk) => chunk.code)
      .join('\n');

    expect(code).toContain('annotatedSideEffect');
    expect(code).toContain('annotated();');
  },
});
