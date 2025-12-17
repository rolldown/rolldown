import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteAssetPlugin, viteWasmHelperPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

const root = path.resolve(import.meta.dirname);

export default defineTest({
  config: {
    plugins: [
      viteAssetPlugin({
        root,
        isLib: false,
        isSsr: false,
        isWorker: false,
        urlBase: '',
        publicDir: '',
        decodedBase: '',
        isSkipAssets: false,
        assetsInclude: [],
        assetInlineLimit: 0,
      }),
      viteWasmHelperPlugin({
        decodedBase: '',
        v2: {
          root,
          isLib: false,
          publicDir: '',
          assetInlineLimit: 0,
        },
      }),
    ],
  },
  async afterTest(output) {
    expect(output.output[1].fileName).toBe('assets/add-Bodj1WnG.wasm');
    expect(output.output[0].modules['\0vite/wasm-helper.js']).toBeDefined();
    expect(
      Object.keys(output.output[0].modules).find(v =>
        v.endsWith('add.wasm?init')
      ),
    ).toBeDefined();
  },
});
