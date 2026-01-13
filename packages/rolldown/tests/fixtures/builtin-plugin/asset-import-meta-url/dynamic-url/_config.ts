import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteAssetImportMetaUrlPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      viteAssetImportMetaUrlPlugin({
        root: '',
        isLib: false,
        publicDir: '',
        clientEntry: '',
        assetInlineLimit: 0,
        tryFsResolve: () => void 0,
        assetResolver: async () => void 0,
      }),
    ],
  },
  async afterTest(output) {
    for (const chunk of output.output) {
      if (chunk.type === 'chunk') {
        await expect(chunk.code).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'main.js.snap'),
        );
        break;
      }
    }
  },
});
