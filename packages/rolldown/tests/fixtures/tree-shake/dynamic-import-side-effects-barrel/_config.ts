import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    platform: 'browser',
    treeshake: true,
  },
  afterTest: (output) => {
    const chunks = output.output.filter((chunk): chunk is OutputChunk => chunk.type === 'chunk');
    const iconsChunk = chunks.find((chunk) => chunk.code.includes('UsedIcon'));
    expect(iconsChunk?.code).toContain('UsedIcon');
    expect(iconsChunk?.code).not.toContain('UnusedIcon');
  },
});
