import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { SourceMapConsumer, type RawSourceMap } from 'source-map-js';
import { expect } from 'vitest';

function getLocation(code: string, search: string): { line: number; column: number } {
  const index = code.indexOf(search);
  expect(index).toBeGreaterThanOrEqual(0);
  const before = code.slice(0, index);
  const lines = before.split('\n');
  return { line: lines.length, column: lines.at(-1)!.length };
}

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'explicit-unmapped-boundary',
        transform(code, id) {
          if (!id.endsWith('/main.js')) {
            return;
          }
          return {
            code: 'foo(); injected();',
            map: {
              version: 3,
              names: [],
              sources: [id],
              sourcesContent: [code],
              // Column 0 maps to the input. Column 5 starts an explicitly unmapped range.
              mappings: 'AAAA,K',
            },
          };
        },
      },
    ],
  },
  async afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    const consumer = new SourceMapConsumer(JSON.parse(JSON.stringify(chunk.map)) as RawSourceMap);
    const mapped = consumer.originalPositionFor(getLocation(chunk.code, 'foo()'));
    const unmapped = consumer.originalPositionFor(getLocation(chunk.code, 'injected()'));

    expect(mapped.source?.endsWith('main.js')).toBe(true);
    expect(unmapped.source).toBeNull();
  },
});
