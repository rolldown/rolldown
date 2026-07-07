import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputSourcemapFilenames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js', 'nested.js'],
    output: {
      sourcemap: true,
      entryFileNames: (chunk) => (chunk.name === 'nested' ? 'entries/[name].js' : '[name].js'),
      sourcemapFileNames: 'maps/[name].js.map',
    },
  },
  afterTest: (output) => {
    expect(getOutputSourcemapFilenames(output)).toStrictEqual([
      'maps/main.js.map',
      'maps/nested.js.map',
    ]);

    const urlOf = (code: string) => /\/\/# sourceMappingURL=(\S+)/.exec(code)?.[1];
    const chunk = (fileName: string) =>
      output.output.find((o) => o.type === 'chunk' && o.fileName === fileName) as OutputChunk;

    // The sourceMappingURL comment must resolve from the chunk's location to the emitted map.
    expect(urlOf(chunk('main.js').code)).toBe('maps/main.js.map');
    expect(urlOf(chunk('entries/nested.js').code)).toBe('../maps/nested.js.map');
  },
});
