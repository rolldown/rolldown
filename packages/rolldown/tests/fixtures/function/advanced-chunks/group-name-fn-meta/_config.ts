import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-meta',
        transform(_code, id) {
          if (id.includes('lib-a')) {
            return { meta: { group: 'vendor' } };
          }
        },
      },
    ],
    output: {
      codeSplitting: {
        groups: [
          {
            name(id, ctx) {
              const info = ctx.getModuleInfo(id);
              if (info?.meta?.group === 'vendor') {
                return 'vendor';
              }
              if (/node_modules/.test(id)) {
                return 'libs';
              }
              return null;
            },
          },
        ],
      },
    },
  },
  afterTest(output) {
    const vendorChunk = output.output.find(
      (chunk) => chunk.type === 'chunk' && chunk.fileName.startsWith('vendor'),
    );
    expect(vendorChunk, 'expected a chunk named "vendor" from meta-based routing').toBeDefined();

    const libsChunk = output.output.find(
      (chunk) => chunk.type === 'chunk' && chunk.fileName.startsWith('libs'),
    );
    expect(libsChunk, 'expected a chunk named "libs" for non-meta modules').toBeDefined();
  },
});
