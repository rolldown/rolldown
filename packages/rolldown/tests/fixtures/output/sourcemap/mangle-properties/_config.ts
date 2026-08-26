import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

function locate(code: string, text: string) {
  const offset = code.indexOf(text);
  expect(offset).toBeGreaterThanOrEqual(0);
  const precedingLines = code.slice(0, offset).split('\n');
  return {
    line: precedingLines.length,
    column: precedingLines.at(-1)!.length,
  };
}

export default defineTest({
  config: {
    input: 'main.js',
    output: {
      sourcemap: true,
      minify: {
        compress: false,
        mangle: false,
        codegen: false,
        mangleProps: {
          include: /^_shared$/,
          cache: { _shared: 'x' },
        },
      },
    },
  },
  afterTest(output) {
    expect(output.mangleCache).toEqual({ _shared: 'x' });
    const chunks = output.output.filter((item) => item.type === 'chunk');
    expect(chunks).toHaveLength(1);

    const [chunk] = chunks;
    expect(chunk.map).toBeDefined();
    expect(chunk.code).toContain('input.x');

    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    expect(originalPositionFor(map, locate(chunk.code, 'marker'))).toMatchObject({
      source: expect.stringMatching(/main\.js$/),
      line: 1,
      column: 27,
    });
  },
});
