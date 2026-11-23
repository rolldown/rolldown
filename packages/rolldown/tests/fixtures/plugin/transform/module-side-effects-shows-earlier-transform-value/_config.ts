import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const dep = path.join(import.meta.dirname, 'dep.js');
const seen: unknown[] = [];

// A `transform` hook sees the value that an earlier `transform` hook of the module returned.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'first',
        transform(code, id) {
          if (id === dep) return { code, moduleSideEffects: false };
        },
      },
      {
        name: 'second',
        transform(_code, id) {
          if (id === dep) seen.push(this.getModuleInfo(id)!.moduleSideEffects);
        },
      },
    ],
  },
  afterTest(output) {
    expect(seen).toStrictEqual([false]);
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).not.toContain('DEP_EFFECT');
  },
});
