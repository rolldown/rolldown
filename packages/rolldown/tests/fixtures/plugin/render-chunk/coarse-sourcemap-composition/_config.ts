import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

function locate(code: string, text: string) {
  const offset = code.indexOf(text);
  expect(
    offset,
    `Could not find ${JSON.stringify(text)} in the generated chunk`,
  ).toBeGreaterThanOrEqual(0);
  const precedingLines = code.slice(0, offset).split('\n');
  return {
    line: precedingLines.length,
    column: precedingLines.at(-1)!.length,
  };
}

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'coarse-render-chunk-map',
        renderChunk(code, chunk) {
          const lineCount = code.split('\n').length;
          return {
            code,
            map: {
              version: 3,
              file: chunk.fileName,
              names: [],
              sources: [chunk.fileName],
              sourcesContent: [code],
              // One column-0 identity token per line. Composition must retain a line even when
              // the preceding detailed map's first token starts after column 0.
              mappings: Array.from({ length: lineCount }, (_, index) =>
                index === 0 ? 'AAAA' : 'AACA',
              ).join(';'),
            },
          };
        },
      },
    ],
  },
  async afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    const expectedMappings = [
      ['globalThis.side = 1', { line: 2, column: 2 }],
      ['return 42', { line: 3, column: 2 }],
    ] as const;

    for (const [statement, expected] of expectedMappings) {
      const original = originalPositionFor(map, locate(chunk.code, statement));
      expect(
        original,
        `Incorrect original position for ${JSON.stringify(statement)}`,
      ).toMatchObject({
        source: expect.stringMatching(/main\.js$/),
        ...expected,
      });
    }
  },
});
