import { globSync } from 'glob';
import fs from 'node:fs';
import nodePath from 'node:path';
import pkgJson from './package.json' with { type: 'json' };
import { colors } from './src/cli/colors';
import {
  defineConfig,
  OutputOptions,
  type Plugin,
  rolldown,
} from './src/index';

const IS_RELEASING_CI = !!process.env.RELEASING;
const IS_BUILD_WASI_PKG = !!process.env.WASI_PKG;

const outputDir = IS_BUILD_WASI_PKG
  ? nodePath.resolve(__dirname, '../wasi/dist')
  : nodePath.resolve(__dirname, 'dist');

const shared = defineConfig({
  input: {
    index: './src/index',
    cli: './src/cli/index',
    'parallel-plugin': './src/parallel-plugin',
    'parallel-plugin-worker': './src/parallel-plugin-worker',
    'experimental-index': './src/experimental-index',
    'parse-ast-index': './src/parse-ast-index',
  },
  platform: 'node',
  resolve: {
    extensions: ['.js', '.cjs', '.mjs', '.ts'],
  },
  external: [
    /rolldown-binding\..*\.node/,
    /rolldown-binding\..*\.wasm/,
    /@rolldown\/binding-.*/,
    /\.\/rolldown-binding\.wasi\.cjs/,
    // some dependencies, e.g. zod, cannot be inlined because their types
    // are used in public APIs
    ...Object.keys(pkgJson.dependencies),
  ],
});

const configs = defineConfig([
  {
    ...shared,
    output: {
      dir: outputDir,
      format: 'esm',
      entryFileNames: '[name].mjs',
      chunkFileNames: 'shared/[name]-[hash].mjs',
    },
    plugins: [
      {
        name: 'shim',
        buildEnd() {
          // wasm build rely on `.node` binaries. But we don't want to copy `.node` files
          // to the dist folder, so we need to distinguish between `.wasm` and `.node` files.
          const wasmFiles = globSync(['./src/rolldown-binding.*.wasm'], {
            absolute: true,
          });

          const isWasmBuild = wasmFiles.length > 0;

          const nodeFiles = globSync(['./src/rolldown-binding.*.node'], {
            absolute: true,
          });

          const wasiShims = globSync(
            ['./src/*.wasi.js', './src/*.wasi.cjs', './src/*.mjs'],
            {
              absolute: true,
            },
          );
          // Binary build is on the separate step on CI
          if (
            !process.env.CI &&
            wasmFiles.length === 0 &&
            nodeFiles.length === 0
          ) {
            throw new Error('No binary files found');
          }

          const copyTo = nodePath.resolve(outputDir);
          fs.mkdirSync(copyTo, { recursive: true });

          if (!IS_RELEASING_CI) {
            // Released `rolldown` package import binary via `@rolldown/binding-<platform>` packages.
            // There's no need to copy binary files to dist folder.

            if (isWasmBuild) {
              // Move the binary file to dist
              wasmFiles.forEach((file) => {
                const fileName = nodePath.basename(file);
                if (IS_BUILD_WASI_PKG && fileName.includes('debug')) {
                  // NAPI-RS now generates a debug wasm binary no matter how and we don't want to ship it to npm.
                  console.log(colors.yellow('[build:done]'), 'Skipping', file);
                } else {
                  console.log(
                    colors.green('[build:done]'),
                    'Copying',
                    file,
                    `to ${copyTo}`,
                  );
                  fs.cpSync(file, nodePath.join(copyTo, fileName), {
                    recursive: true,
                    force: true,
                  });
                }
                console.log(colors.green('[build:done]'), `Cleaning ${file}`);
                try {
                  // GitHub windows runner emits `operation not permitted` error, most likely because of the file is still in use.
                  // We could safely ignore the error.
                  fs.rmSync(file, { recursive: true, force: true });
                } catch {}
              });
            } else {
              // Move the binary file to dist
              nodeFiles.forEach((file) => {
                const fileName = nodePath.basename(file);
                console.log(
                  colors.green('[build:done]'),
                  'Copying',
                  file,
                  `to ${copyTo}`,
                );
                fs.cpSync(file, nodePath.join(copyTo, fileName), {
                  recursive: true,
                  force: true,
                });
                console.log(colors.green('[build:done]'), `Cleaning ${file}`);
              });
            }

            wasiShims.forEach((file) => {
              const fileName = nodePath.basename(file);
              console.log(
                colors.green('[build:done]'),
                'Copying',
                file,
                'to ./dist/shared',
              );
              fs.cpSync(file, nodePath.join(copyTo, fileName), {
                recursive: true,
                force: true,
              });
            });
          }

          // Copy binding types and rollup types to dist
          const distTypesDir = nodePath.resolve(outputDir, 'types');
          fs.mkdirSync(distTypesDir, { recursive: true });
          const types = globSync(['./src/*.d.ts'], {
            absolute: true,
          });
          types.forEach((file) => {
            const fileName = nodePath.basename(file);
            console.log(
              colors.green('[build:done]'),
              'Copying',
              file,
              'to ./dist/shared',
            );
            fs.cpSync(file, nodePath.join(distTypesDir, fileName), {
              recursive: true,
              force: true,
            });
          });
        },
      },
      patchBindingJs(),
    ],
  },
  {
    ...shared,
    plugins: [
      {
        name: 'shim-import-meta',
        transform: {
          filter: {
            code: {
              include: ['import.meta.resolve'],
            },
          },
          handler(code, id) {
            if (id.endsWith('.ts') && code.includes('import.meta.resolve')) {
              return code.replace('import.meta.resolve', 'undefined');
            }
          },
        },
      },
      patchBindingJs(),
    ],
    output: {
      dir: outputDir,
      format: 'cjs',
      entryFileNames: '[name].cjs',
      chunkFileNames: 'shared/[name]-[hash].cjs',
    },
  },
]);

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
            .replace('require = createRequire(__filename)', '')
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

(async () => {
  for (const config of configs) {
    await (await rolldown(config)).write(config.output as OutputOptions);
  }
})();
