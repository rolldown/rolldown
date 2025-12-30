import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { type Plugin } from './src/index';

interface CopyAddonPluginOptions {
  isCI: boolean;
  isReleasingPkgInCI: boolean;
  desireWasmFiles: boolean;
}

const WASM_BINDING_FILES = [
  'rolldown-binding.wasm32-wasi.wasm',
  'rolldown-binding.wasi-browser.js',
  'rolldown-binding.wasi.cjs',
];

const WASM_WORKER_FILES = ['wasi-worker-browser.mjs', 'wasi-worker.mjs'];

export const CopyAddonPlugin = ({
  isCI,
  isReleasingPkgInCI,
  desireWasmFiles,
}: CopyAddonPluginOptions): Plugin => {
  const addonsToEmit = new Map<string, string>();
  let outputDir = '';
  if (desireWasmFiles) {
    const baseDir = join(fileURLToPath(import.meta.url), '..');
    const distDir = join(baseDir, 'dist');
    const srcDir = join(baseDir, 'src');

    // Generated binding files are in dist/
    for (const file of WASM_BINDING_FILES) {
      addonsToEmit.set(join(distDir, file), '');
    }

    // Worker source files are in src/
    for (const file of WASM_WORKER_FILES) {
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
