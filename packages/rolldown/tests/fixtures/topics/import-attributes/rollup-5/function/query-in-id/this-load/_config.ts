import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const VIRTUAL_ID = '\0virtual:asset?width=100&height=200';

// Rollup 5 calls `this.load` in `buildEnd`, but Rolldown only supports it while modules are
// loading, so this uses `transform`. Rollup 5 also checks `info.rawId` and `info.attributes`, which
// Rolldown's `ModuleInfo` does not have yet.
export default defineTest({
  config: {
    input: main,
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test',
        load(id, { rawId, attributes }) {
          if (id === VIRTUAL_ID) {
            expect(rawId).toBe(VIRTUAL_ID);
            expect(attributes).toEqual({});
            return 'export const v = 1';
          }
        },
        async transform(_code, id) {
          if (id !== main) return;
          const info = await this.load({ id: VIRTUAL_ID });
          expect(info.id).toBe(VIRTUAL_ID);
        },
      },
    ],
  },
});
