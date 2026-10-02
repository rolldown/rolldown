import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { type Plugin } from './src/index';

interface CopyAddonPluginOptions {
  isCI: boolean;
  isReleasingPkgInCI: boolean;
  desireWasmFiles: boolean;
  wasmSingleThread?: boolean;
  workerdPackageApi?: boolean;
}

// Per-flavor WASI artifact sets, named distinctly so both can co-exist under
// packages/rolldown/src: threaded keeps the legacy `wasm32-wasi`/`wasi` stems,
// single-thread uses `wasm32-wasip1`/`wasip1` and ships no worker scripts.
const WASM_FILE_LIST_THREADED = [
  'rolldown-binding.wasm32-wasi.wasm',
  'rolldown-binding.wasi-browser.js',
  'rolldown-binding.wasi.cjs',
  'wasi-worker-browser.mjs',
  'wasi-worker.mjs',
];

const WASM_FILE_LIST_SINGLE = [
  'rolldown-binding.wasm32-wasip1.wasm',
  'rolldown-binding.wasip1-browser.js',
  'rolldown-binding.wasip1-deferred.js',
  'rolldown-binding.wasip1.cjs',
];

export const CopyAddonPlugin = ({
  isCI,
  isReleasingPkgInCI,
  desireWasmFiles,
  wasmSingleThread,
  workerdPackageApi,
}: CopyAddonPluginOptions): Plugin => {
  const addonsToEmit = new Map<string, string>();
  let outputDir = '';
  if (desireWasmFiles) {
    const srcDir = join(fileURLToPath(import.meta.url), '..', 'src');
    const wasmFileList = wasmSingleThread ? WASM_FILE_LIST_SINGLE : WASM_FILE_LIST_THREADED;
    for (const file of wasmFileList) {
      addonsToEmit.set(join(srcDir, file), '');
    }
    if (workerdPackageApi) {
      addonsToEmit.set(join(srcDir, 'workerd-wasm.d.ts'), '');
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
              await this.fs
                .stat(addonPath)
                .then((s) => s.isFile())
                .catch(() => false)
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
