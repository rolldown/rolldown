import colors from 'ansis';
import { globSync } from 'glob';
import fs from 'node:fs';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';
import { dts } from 'rolldown-plugin-dts';
import { build, BuildOptions, type Plugin } from './src/index';

const isCI = !!process.env.CI;
const isReleasingCI = !!process.env.RELEASING;
const __dirname = nodePath.join(fileURLToPath(import.meta.url), '..');

// In `@rolldown/browser`, there will be three builds:
// - CJS and ESM for Node (used in StackBlitz / WebContainers)
// - ESM for browser bundlers (used in Vite and running in the browser)
const isBrowserPkg = !!process.env.BROWSER_PKG;

const pkgRoot = isBrowserPkg
  ? nodePath.resolve(__dirname, '../browser')
  : __dirname;
const outputDir = nodePath.resolve(pkgRoot, 'dist');
const pkgJson = JSON.parse(
  fs.readFileSync(nodePath.resolve(pkgRoot, 'package.json'), 'utf-8'),
);

const bindingFile = nodePath.resolve('src/binding.js');
const bindingDtsFile = nodePath.resolve('src/binding.d.ts');
const bindingFileWasi = nodePath.resolve('src/rolldown-binding.wasi.cjs');
const bindingFileWasiBrowser = nodePath.resolve(
  'src/rolldown-binding.wasi-browser.js',
);

const configs: BuildOptions[] = [
  withShared({
    plugins: [patchBindingJs(), dts()],
    output: {
      dir: outputDir,
      format: 'esm',
      entryFileNames: `[name].mjs`,
      chunkFileNames: `shared/[name]-[hash].mjs`,
    },
  }),
  withShared({
    plugins: [shimImportMeta(), patchBindingJs()],
    output: {
      dir: outputDir,
      format: 'cjs',
      entryFileNames: '[name].cjs',
      chunkFileNames: 'shared/[name]-[hash].cjs',
    },
  }),
  withShared({
    plugins: [dts({ emitDtsOnly: true })],
    output: {
      dir: outputDir,
      format: 'esm',
      entryFileNames: '[name].cjs',
      chunkFileNames: 'shared/[name]-[hash].cjs',
    },
  }),
];

if (isBrowserPkg) {
  configs.push(
    withShared({
      browserBuild: true,
      output: {
        dir: outputDir,
        format: 'esm',
        entryFileNames: '[name].browser.mjs',
      },
    }),
  );
}

(async () => {
  for (const config of configs) {
    await build(config);
  }
  copy();
})();

function withShared(
  { browserBuild: isBrowserBuild, ...options }:
    & { browserBuild?: boolean }
    & BuildOptions,
): BuildOptions {
  return {
    input: {
      index: './src/index',
      'experimental-index': './src/experimental-index',
      ...!isBrowserBuild
        ? {
          cli: './src/cli/index',
          'parallel-plugin': './src/parallel-plugin',
          'parallel-plugin-worker': './src/parallel-plugin-worker',
          'filter-index': './src/filter-index',
          'parse-ast-index': './src/parse-ast-index',
        }
        : {},
    },
    platform: isBrowserBuild ? 'browser' : 'node',
    resolve: {
      extensions: ['.js', '.cjs', '.mjs', '.ts'],
    },
    external: [
      /rolldown-binding\..*\.node/,
      /rolldown-binding\..*\.wasm/,
      /@rolldown\/binding-.*/,
      /\.\/rolldown-binding\.wasi\.cjs/,
      ...Object.keys(pkgJson.dependencies ?? {}),
    ],
    define: {
      'import.meta.browserBuild': String(isBrowserBuild),
    },
    ...options,
    plugins: [
      isBrowserPkg && resolveWasiBinding(isBrowserBuild),
      isBrowserBuild && removeBuiltModules(),
      options.plugins,
    ],
  };
}

// browser package only
// alias binding file to rolldown-binding.wasi.js and mark it as external
// alias its dts file to rolldown-binding.d.ts without external
function resolveWasiBinding(isBrowserBuild?: boolean): Plugin {
  return {
    name: 'resolve-wasi-binding',
    resolveId: {
      filter: { id: /\bbinding\b/ },
      async handler(id, importer, options) {
        const resolution = await this.resolve(id, importer, options);

        if (resolution?.id === bindingFile) {
          const mod = importer && this.getModuleInfo(importer);
          // if importer is a dts file
          const dtsFile = mod ? mod.meta?.dtsFile : false;

          if (dtsFile) {
            // link to src/binding.d.ts
            return { id: bindingDtsFile };
          } else {
            const id = isBrowserBuild
              ? bindingFileWasiBrowser
              : bindingFileWasi;
            return { id, external: 'relative' };
          }
        }

        return resolution;
      },
    },
  };
}

function removeBuiltModules(): Plugin {
  return {
    name: 'remove-built-modules',
    resolveId: {
      filter: { id: /node:/ },
      handler(id, importer) {
        if (id === 'node:path') {
          return this.resolve('pathe');
        }
        if (
          id === 'node:os' || id === 'node:worker_threads' || id === 'node:url'
        ) {
          // conditional import
          return { id, external: true, moduleSideEffects: false };
        }
        throw new Error(`Unresolved module: ${id} from ${importer}`);
      },
    },
  };
}

function shimImportMeta(): Plugin {
  return {
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
  };
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

function copy() {
  // wasm build rely on `.node` binaries. But we don't want to copy `.node` files
  // to the dist folder, so we need to distinguish between `.wasm` and `.node` files.
  const wasmFiles = globSync('./src/rolldown-binding.*.wasm', {
    absolute: true,
  });
  const nodeFiles = globSync('./src/rolldown-binding.*.node', {
    absolute: true,
  });

  // Binary build is on the separate step on CI
  if (!isCI) {
    if (isBrowserPkg && !wasmFiles.length) {
      throw new Error('No WASM files found.');
    }
    if (!isBrowserPkg && !nodeFiles.length) {
      throw new Error('No Node files found.');
    }
  }

  const copyTo = nodePath.resolve(outputDir);
  fs.mkdirSync(copyTo, { recursive: true });

  if (!isReleasingCI) {
    // Released `rolldown` package import binary via `@rolldown/binding-<platform>` packages.
    // There's no need to copy binary files to dist folder.

    if (isBrowserPkg) {
      // Move the wasm file to dist
      wasmFiles.forEach((file) => {
        const fileName = nodePath.basename(file);
        if (isBrowserPkg && fileName.includes('debug')) {
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

      const browserShims = globSync(
        './src/*wasi*js',
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
  }
}
