import { green, yellow } from 'ansis';
import { cpSync, globSync } from 'node:fs';
import path from 'node:path';
import { defineConfig, logger, type ResolvedOptions } from 'tsdown';
import {
  InternalModuleFormat,
  type OutputOptions,
  type Plugin,
} from './src/index';

const isCI = !!process.env.CI;
const isReleasingCI = !!process.env.RELEASING;

// In `@rolldown/browser`, there will be three builds:
// - CJS and ESM for Node (used in StackBlitz / WebContainers)
// - ESM for browser bundlers (used in Vite and running in the browser)
const isBrowserPkg = !!process.env.BROWSER_PKG;
const isBrowserBuild = false;

export default defineConfig({
  entry: {
    index: './src/index',
    'experimental-index': './src/experimental-index',
    ...!isBrowserBuild
      ? {
        cli: './src/cli/index',
        config: './src/config',
        'parallel-plugin': './src/parallel-plugin',
        'parallel-plugin-worker': './src/parallel-plugin-worker',
        'filter-index': './src/filter-index',
        'parse-ast-index': './src/parse-ast-index',
      }
      : {},
  },
  platform: isBrowserBuild ? 'browser' : 'node',
  format: ['esm', 'cjs'],
  fixedExtension: true,
  external: [
    /rolldown-binding\..*\.node/,
    /@rolldown\/binding-.*/,
    /\.\/rolldown-binding\.wasi\.cjs/,
  ],
  define: {
    'import.meta.browserBuild': String(isBrowserBuild),
  },
  plugins: [patchBindingJs()],

  outputOptions(options: OutputOptions, format: InternalModuleFormat) {
    if (format === 'cjs') {
      options.chunkFileNames = 'shared/[name]-[hash].cjs';
    } else {
      options.chunkFileNames = 'shared/[name]-[hash].mjs';
    }
  },

  onSuccess: copy,
});

function patchBindingJs(): Plugin {
  return {
    name: 'patch-binding-js',
    transform: {
      filter: {
        id: 'src/binding.js',
      },
      handler(code) {
        return (
          code
            // strip off unneeded createRequire in cjs, which breaks mjs
            .replace('const require = createRequire(import.meta.url)', '')
            // inject binding auto download fallback for webcontainer
            .replace(
              '\nif (!nativeBinding) {',
              (s) =>
                `
if (!nativeBinding && globalThis.process?.versions?.["webcontainer"]) {
  try {
    nativeBinding = require('./webcontainer-fallback.js');
  } catch (err) {
    loadErrors.push(err)
  }
}
` + s,
            )
        );
      },
    },
  };
}

function copy(config: ResolvedOptions) {
  // wasm build rely on `.node` binaries. But we don't want to copy `.node` files
  // to the dist folder, so we need to distinguish between `.wasm` and `.node` files.
  const wasmFiles = globSync('./src/rolldown-binding.*.wasm', {
    withFileTypes: false,
  });

  const napiFiles = globSync('./src/rolldown-binding.*.node', {
    withFileTypes: false,
  });

  // Binary build is on the separate step on CI
  if (!isCI) {
    if (isBrowserPkg && !wasmFiles.length) {
      throw new Error('No WASM files found.');
    }
    if (!isBrowserPkg && !napiFiles.length) {
      throw new Error('No NAPI Node files found.');
    }
  }

  if (!isReleasingCI) {
    // Released `rolldown` package import binary via `@rolldown/binding-<platform>` packages.
    // There's no need to copy binary files to dist folder.

    if (isBrowserPkg) {
      // Move the wasm file to dist
      for (const file of wasmFiles) {
        const fileName = path.basename(file);
        if (isBrowserPkg && fileName.includes('debug')) {
          // NAPI-RS now generates a debug wasm binary no matter how and we don't want to ship it to npm.
          logger.info(yellow`[copy]`, 'Skipping', file);
        } else {
          logger.info(
            green`[copy]`,
            'Copying',
            file,
            `to ${config.outDir}`,
          );
          cpSync(file, path.join(config.outDir, fileName));
        }
      }

      const browserShims = globSync(
        './src/*wasi*js',
      );
      for (const file of browserShims) {
        const fileName = path.basename(file);
        logger.info(
          green`[copy]`,
          'Copying',
          file,
          `to ${config.outDir}`,
        );
        cpSync(file, path.join(config.outDir, fileName));
      }
    } else {
      // Move the napi node file to dist
      for (const file of napiFiles) {
        const fileName = path.basename(file);
        logger.info(
          green`[copy]`,
          'Copying',
          file,
          `to ${config.outDir}`,
        );
        cpSync(file, path.join(config.outDir, fileName));
      }
    }
  }
}
