import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const libAId = /[\\/]lib-a\.js$/;
const libBId = /[\\/]lib-b\.js$/;

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-meta',
        transform(_code, id) {
          if (libAId.test(id)) {
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
              if (libAId.test(id)) {
                expect(info?.meta).toMatchObject({ group: 'vendor' });
              }
              if (info?.meta?.group === 'vendor') {
                return 'vendor';
              }
              if (libBId.test(id)) {
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
    expect(vendorChunk, 'expected lib-a in a meta-selected "vendor" chunk').toMatchObject({
      type: 'chunk',
      moduleIds: [expect.stringMatching(libAId)],
    });

    const libsChunk = output.output.find(
      (chunk) => chunk.type === 'chunk' && chunk.fileName.startsWith('libs'),
    );
    expect(libsChunk, 'expected lib-b in the fallback "libs" chunk').toMatchObject({
      type: 'chunk',
      moduleIds: [expect.stringMatching(libBId)],
    });
  },
});
