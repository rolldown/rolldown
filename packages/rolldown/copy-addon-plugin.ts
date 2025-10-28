import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { type Plugin } from './src/index';

interface CopyAddonPluginOptions {
  isCI: boolean;
  isReleasingPkgInCI: boolean;
  desireWasmFiles: boolean;
}

const WASM_FILE_LIST = [
  'rolldown-binding.wasm32-wasi.wasm',
  'rolldown-binding.wasi-browser.js',
  'rolldown-binding.wasi.cjs',
  'wasi-worker-browser.mjs',
  'wasi-worker.mjs',
];

export const CopyAddonPlugin = (
  { isCI, isReleasingPkgInCI, desireWasmFiles }: CopyAddonPluginOptions,
): Plugin => {
  const addonsToEmit = new Map<string, string>();
  let outputDir = '';
  if (desireWasmFiles) {
    const srcDir = join(fileURLToPath(import.meta.url), '..', 'src');
    for (const file of WASM_FILE_LIST) {
      addonsToEmit.set(join(srcDir, file), '');
    }
  }
  return {
    name: 'copy-addon',
    outputOptions(options) {
      outputDir = options.dir ?? '';
    },
    resolveId: {
      filter: {
        id: /binding/,
      },
      async handler(id, importer) {
        if (id.endsWith('.node')) {
          if (desireWasmFiles) {
            return {
              id,
              external: true,
            };
          }
          if (importer) {
            const addonPath = join(dirname(importer), id);
            if (
              await this.fs.stat(addonPath).then((s) => s.isFile()).catch(() =>
                false
              )
            ) {
              addonsToEmit.set(addonPath, importer);
              return {
                id: addonPath,
                external: true,
              };
            }
          }
        }
      },
    },
    async buildEnd() {
      if (!isCI && addonsToEmit.size === 0) {
        throw new Error('No .node files found');
      }
      if (isReleasingPkgInCI) {
        return;
      }
      for (const addonPath of addonsToEmit.keys()) {
        await this.fs.copyFile(addonPath, join(outputDir, basename(addonPath)));
      }
    },
  };
};
