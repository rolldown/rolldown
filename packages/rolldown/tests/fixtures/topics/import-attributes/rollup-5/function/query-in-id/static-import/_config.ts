import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const VIRTUAL_ID = '\0virtual:asset?width=100&height=200';

// Rollup 5 also checks `info.rawId` and `info.attributes`, which Rolldown's `ModuleInfo` does not
// have yet.
export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test',
        resolveId(source) {
          if (source === 'virtual') return VIRTUAL_ID;
        },
        load(id) {
          if (id === VIRTUAL_ID) return 'export const v = 1';
        },
        buildEnd() {
          const info = this.getModuleInfo(VIRTUAL_ID);
          expect(info).toBeTruthy();
          expect(info!.id).toBe(VIRTUAL_ID);
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const { v } = await import('./dist/main.js');
    expect(v).toBe(1);
  },
});
