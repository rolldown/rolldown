import colors from 'ansis';
import { globSync } from 'glob';
import fs from 'node:fs';
import nodePath from 'node:path';
import pkgJson from './package.json' with { type: 'json' };
import { build, BuildOptions, defineConfig, type Plugin } from './src/index';

const isCI = !!process.env.CI;
const isReleasingCI = !!process.env.RELEASING;
const isBrowserBuild = !!process.env.BROWSER_PKG;

const outputDir = isBrowserBuild
  ? nodePath.resolve(__dirname, '../browser/dist')
  : nodePath.resolve(__dirname, 'dist');

const bindingFile = nodePath.resolve('src/binding.js');
const bindingFileWasiBrowser = nodePath.resolve(
  'src/rolldown-binding.wasi-browser.js',
);

const shared = defineConfig({
  input: {
    index: './src/index',
    ...isBrowserBuild ? {} : {
      cli: './src/cli/index',
      'parallel-plugin': './src/parallel-plugin',
      'parallel-plugin-worker': './src/parallel-plugin-worker',
      'experimental-index': './src/experimental-index',
      'parse-ast-index': './src/parse-ast-index',
    },
  },
  platform: isBrowserBuild ? 'browser' : 'node',
  resolve: {
    extensions: ['.js', '.cjs', '.mjs', '.ts'],
    alias: {
      'node:path': 'pathe',
      ...(isBrowserBuild ? { [bindingFile]: bindingFileWasiBrowser } : {}),
    },
  },
  external: [
    /rolldown-binding\..*\.node/,
    /rolldown-binding\..*\.wasm/,
    /@rolldown\/binding-.*/,
    /\.\/rolldown-binding\.wasi\.cjs/,
    // some dependencies, e.g. zod, cannot be inlined because their types
    // are used in public APIs
    ...(isBrowserBuild
      ? []
      : Object.keys(pkgJson.dependencies)),
    bindingFileWasiBrowser,
  ],
  define: {
    'import.meta.browserBuild': String(isBrowserBuild),
  },
  plugins: [
    isBrowserBuild && {
      name: 'remove-built-modules',
      resolveId(id) {
        if (id === 'node:os' || id === 'node:worker_threads') {
          return { id, external: true, moduleSideEffects: false };
        }
      },
    },
  ],
});

const esmSuffix = isBrowserBuild ? 'js' : 'mjs';

const configs = defineConfig([
  {
    ...shared,
    output: {
      dir: outputDir,
      format: 'esm',
      entryFileNames: `[name].${esmSuffix}`,
      chunkFileNames: `shared/[name]-[hash].${esmSuffix}`,
    },
    plugins: [
      shared.plugins,
      {
        name: 'shim',
        buildEnd() {
          // wasm build rely on `.node` binaries. But we don't want to copy `.node` files
          // to the dist folder, so we need to distinguish between `.wasm` and `.node` files.
          const wasmFiles = globSync(['./src/rolldown-binding.*.wasm'], {
            absolute: true,
          });
          const isWasmBuild = wasmFiles.length;

          const nodeFiles = globSync(['./src/rolldown-binding.*.node'], {
            absolute: true,
          });

          // Binary build is on the separate step on CI
          if (!isCI && !wasmFiles.length && !nodeFiles.length) {
            throw new Error('No binary files found');
          }

          const copyTo = nodePath.resolve(outputDir);
          fs.mkdirSync(copyTo, { recursive: true });

          if (!isReleasingCI) {
            // Released `rolldown` package import binary via `@rolldown/binding-<platform>` packages.
            // There's no need to copy binary files to dist folder.

            if (isWasmBuild) {
              // Move the binary file to dist
              wasmFiles.forEach((file) => {
                const fileName = nodePath.basename(file);
                if (isBrowserBuild && fileName.includes('debug')) {
                  // NAPI-RS now generates a debug wasm binary no matter how and we don't want to ship it to npm.
                  console.log(colors.yellow('[build:done]'), 'Skipping', file);
                } else {
                  console.log(
                    colors.green('[build:done]'),
                    'Copying',
                    file,
                    `to ${copyTo}`,
                  );
                  fs.cpSync(file, nodePath.join(copyTo, fileName));
                }
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
                fs.cpSync(file, nodePath.join(copyTo, fileName));
              });
            }

            if (isBrowserBuild) {
              const browserShims = globSync(
                './src/*-browser.*js',
                { absolute: true },
              );
              browserShims.forEach((file) => {
                const fileName = nodePath.basename(file);
                console.log(
                  colors.green('[build:done]'),
                  'Copying',
                  file,
                  `to ${copyTo}`,
                );
                fs.cpSync(file, nodePath.join(copyTo, fileName));
              });
            }
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
              `to ${distTypesDir}`,
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
]);

if (!isBrowserBuild) {
  configs.push(
    {
      ...shared,
      plugins: [
        shared.plugins,
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
  );
}

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
    await build(config as BuildOptions);
  }
})();
