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
    experimental: {
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
      minify: true,
    },
    plugins: [
      {
        name: 'return-magic-string',
        renderChunk(_code, _chunk, _options, meta) {
          meta.magicString!.prepend('globalThis.first = true;\n');
          return meta.magicString!;
        },
      },
      {
        name: 'return-magic-string-as-code',
        renderChunk(_code, _chunk, _options, meta) {
          meta.magicString!.prepend('globalThis.second = true;\n');
          return { code: meta.magicString! };
        },
      },
    ],
  },
  async afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code).toContain('globalThis.first');
    expect(chunk.code).toContain('globalThis.second');

    // Neither hook returns a map, so Rolldown generates one from each MagicString. Those maps
    // sit between the chunk's map and the minifier's map. If they were line-level, every token
    // would resolve to the first token of its line.
    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    const expectedMappings = [
      ['parseFloat', { line: 2, column: 24 }],
      ['isNaN', { line: 3, column: 16 }],
    ] as const;

    for (const [token, expected] of expectedMappings) {
      const original = originalPositionFor(map, locate(chunk.code, token));
      expect(original, `Incorrect original position for ${JSON.stringify(token)}`).toMatchObject({
        source: expect.stringMatching(/main\.js$/),
        ...expected,
      });
    }
  },
});
