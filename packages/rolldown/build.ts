import fs from 'node:fs';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { dts } from 'rolldown-plugin-dts';
import * as ts from 'typescript';

import { CopyAddonPlugin } from './copy-addon-plugin';
import type { BuildOptions, Plugin } from './src/index';
import { build } from './src/index';
import { styleText } from './src/utils/style-text';

const __dirname = nodePath.join(fileURLToPath(import.meta.url), '..');
const bufferPolyfillPath = fileURLToPath(import.meta.resolve('buffer/'));

const buildMeta = (function makeBuildMeta() {
  // Refer to `@rolldown/browser` package.
  // In `@rolldown/browser`, there will be two builds:
  // - ESM for Node (used in StackBlitz / WebContainers)
  // - ESM for browser bundlers (used in Vite and running in the browser)
  type TargetBrowserPkg = 'browser-pkg';

  // Refer to `rolldown` package
  type TargetRolldownPkg = 'rolldown-pkg';

  // Threaded (wasm32-wasip1-threads) and single-thread (wasm32-wasip1) WASI
  // dist builds: the artifact sets have distinct per-flavor names, so each
  // target wires the dist to its own flavor's loaders.
  type TargetRolldownPkgWasi = 'rolldown-pkg-wasi';
  type TargetRolldownPkgWasiSingle = 'rolldown-pkg-wasi-single';

  const target:
    | TargetBrowserPkg
    | TargetRolldownPkg
    | TargetRolldownPkgWasi
    | TargetRolldownPkgWasiSingle = (function determineTarget() {
    switch (process.env.TARGET) {
      case undefined:
      case 'rolldown':
        return 'rolldown-pkg';
      case 'browser':
        return 'browser-pkg';
      case 'rolldown-wasi':
        return 'rolldown-pkg-wasi';
      case 'rolldown-wasi-single':
        return 'rolldown-pkg-wasi-single';
      default:
        console.warn(`Unknown target: ${process.env.TARGET}, defaulting to 'rolldown-pkg'`);
        return 'rolldown-pkg';
    }
  })();

  const pkgRoot = target === 'browser-pkg' ? nodePath.resolve(__dirname, '../browser') : __dirname;

  return {
    isCI: !!process.env.CI,
    isReleasingPkgInCI: !!process.env.RELEASING,
    target,
    pkgRoot,
    buildOutputDir: nodePath.resolve(pkgRoot, 'dist'),
    pkgJson: JSON.parse(fs.readFileSync(nodePath.resolve(pkgRoot, 'package.json'), 'utf-8')),
    desireWasmFiles:
      target === 'browser-pkg' ||
      target === 'rolldown-pkg-wasi' ||
      target === 'rolldown-pkg-wasi-single',
    // `@rolldown/browser` and the wasi-single dist ship the single-thread
    // (wasm32-wasip1) artifact set; only the threaded wasi dist ships the
    // threaded (wasm32-wasi) set.
    wasmSingleThread: target === 'browser-pkg' || target === 'rolldown-pkg-wasi-single',
  };
})();

const bindingFile = nodePath.resolve('src/binding.cjs');
// per-flavor WASI node loaders: threaded keeps the legacy `wasi` stem,
// the single-thread flavor has its own distinct `wasip1` stem
const bindingFileWasi = nodePath.resolve(
  buildMeta.wasmSingleThread ? 'src/rolldown-binding.wasip1.cjs' : 'src/rolldown-binding.wasi.cjs',
);
const bindingFileWasiBrowser = nodePath.resolve(
  buildMeta.wasmSingleThread
    ? 'src/rolldown-binding.wasip1-browser.js'
    : 'src/rolldown-binding.wasi-browser.js',
);
const bindingFileWasiDeferred = nodePath.resolve('src/rolldown-binding.wasip1-deferred.js');
const threadedWasiLoaderArtifactDir = nodePath.resolve('artifacts/threaded-wasi-loaders');
const threadedWasiFiles = {
  binding: nodePath.resolve('src/rolldown-binding.wasi.cjs'),
  browserBinding: nodePath.resolve('src/rolldown-binding.wasi-browser.js'),
  worker: nodePath.resolve('src/wasi-worker.mjs'),
  browserWorker: nodePath.resolve('src/wasi-worker-browser.mjs'),
};
const configs: BuildOptions[] = [
  withShared({
    plugins: [patchBindingJs(), dts(), removeIncludeTagsFromDts()],
    output: {
      dir: buildMeta.buildOutputDir,
      format: 'esm',
      entryFileNames: `[name].mjs`,
      chunkFileNames: `shared/[name]-[hash].mjs`,
    },
  }),
];

if (buildMeta.target === 'browser-pkg') {
  let init = withShared({
    browserBuild: true,
    output: {
      dir: buildMeta.buildOutputDir,
      format: 'esm',
      entryFileNames: '[name].browser.mjs',
    },
  });
  init.transform ??= {};
  init.transform.define = {
    ...init.transform.define,
    // `experimental-index` now dependents on `logger` in cli to emit warning which require `process.env.ROLLDOWN_TEST` to initialize logger correctly.
    // But in browser build, we don't have `process.`, so we polyfill them
    'process.env.ROLLDOWN_TEST': 'false',
  };
  configs.push(init);
}

(async () => {
  // clean up unused files that may be left from previous builds
  fs.rmSync(buildMeta.buildOutputDir, { recursive: true, force: true });
  fs.mkdirSync(buildMeta.buildOutputDir, { recursive: true });

  for (const config of configs) {
    await build(config);
  }
  if (buildMeta.target === 'browser-pkg') {
    await bundleManagedWorkerdLoaders();
    await bundleBrowserWasiLoaders();
    await bundleThreadedWasiLoaders();
  }
  generateRuntimeTypes();
})();

function withShared({
  browserBuild: isBrowserBuild,
  ...options
}: { browserBuild?: boolean } & BuildOptions): BuildOptions {
  return {
    input: {
      index: './src/index',
      'plugins-index': './src/plugins-index',
      'utils-index': './src/utils-index',
      'experimental-index': './src/experimental-index',
      ...(!isBrowserBuild
        ? {
            cli: './src/cli/index',
            config: './src/config',
            'parallel-plugin': './src/parallel-plugin',
            'parallel-plugin-worker': './src/parallel-plugin-worker',
            'filter-index': './src/filter-index',
            'parse-ast-index': './src/parse-ast-index',
            'get-log-filter': './src/get-log-filter',
          }
        : {}),
    },
    platform: isBrowserBuild ? 'browser' : 'node',
    resolve: {
      extensions: ['.js', '.cjs', '.mjs', '.ts'],
    },
    external: [
      /@rolldown\/binding-.*/,
      /rolldown-binding\.(wasi|wasip1)\.cjs/,
      ...Object.keys(buildMeta.pkgJson.dependencies ?? {}),
    ],
    // Do not move this line up or down, it's here for a reason
    ...options,
    plugins: [
      buildMeta.desireWasmFiles && resolveWasiBinding(isBrowserBuild),
      CopyAddonPlugin({
        isCI: buildMeta.isCI,
        isReleasingPkgInCI: buildMeta.isReleasingPkgInCI,
        desireWasmFiles: buildMeta.desireWasmFiles,
        wasmSingleThread: buildMeta.wasmSingleThread,
        workerdPackageApi: buildMeta.target === 'browser-pkg',
      }),
      isBrowserBuild && removeBuiltModules(),
      options.plugins,
    ],
    treeshake: {
      moduleSideEffects: [{ test: /\/signal-exit\//, sideEffects: false }],
    },
    transform: {
      target: 'node22',
      define: {
        'import.meta.browserBuild': String(isBrowserBuild),
        'import.meta.workerdPackageApi': String(buildMeta.target === 'browser-pkg'),
      },
    },
  };
}

// Keep the managed workerd entries self-contained so release staging can use
// the same public factory in @rolldown/browser, the threadless optional
// package, and the generated rolldown/workerd facade.
// See internal-docs/async-runtime/implementation.md.
async function bundleManagedWorkerdLoaders() {
  await build({
    input: nodePath.resolve('src/workerd.ts'),
    platform: 'node',
    resolve: {
      alias: {
        buffer: bufferPolyfillPath,
      },
    },
    output: {
      file: nodePath.join(buildMeta.buildOutputDir, 'workerd.mjs'),
      format: 'esm',
      codeSplitting: false,
    },
    transform: {
      target: 'node22',
    },
  });

  await build({
    input: nodePath.resolve('src/workerd.ts'),
    platform: 'browser',
    resolve: {
      alias: {
        buffer: bufferPolyfillPath,
      },
    },
    output: {
      file: nodePath.join(buildMeta.buildOutputDir, 'workerd.browser.mjs'),
      format: 'esm',
      codeSplitting: false,
    },
    plugins: [removeBuiltModules()],
    transform: {
      target: 'node22',
    },
  });

  await build({
    input: {
      workerd: nodePath.resolve('src/workerd.ts'),
    },
    output: {
      dir: buildMeta.buildOutputDir,
      format: 'esm',
      entryFileNames: '[name].mjs',
      codeSplitting: false,
    },
    plugins: [dts({ emitDtsOnly: true }), removeIncludeTagsFromDts()],
  });
}

// Published browser consumers do not inherit the workspace's pnpm patches.
// Bundle the generated loaders last so every supported package-root condition
// embeds the exact hardened emnapi runtime used to build the release.
// See internal-docs/async-runtime/implementation.md.
async function bundleBrowserWasiLoaders() {
  await build({
    input: bindingFileWasiBrowser,
    platform: 'browser',
    output: {
      file: nodePath.join(buildMeta.buildOutputDir, nodePath.basename(bindingFileWasiBrowser)),
      format: 'esm',
      codeSplitting: false,
    },
    transform: {
      target: 'node22',
    },
  });

  await build({
    input: bindingFileWasi,
    platform: 'node',
    external: [/^node:/, /^@rolldown\/binding-wasm32-wasip1(?:\/|$)/],
    output: {
      file: nodePath.join(buildMeta.buildOutputDir, nodePath.basename(bindingFileWasi)),
      format: 'cjs',
      codeSplitting: false,
    },
    transform: {
      target: 'node22',
    },
  });

  await build({
    input: bindingFileWasiDeferred,
    platform: 'browser',
    resolve: {
      alias: {
        buffer: bufferPolyfillPath,
      },
    },
    output: {
      file: nodePath.join(buildMeta.buildOutputDir, nodePath.basename(bindingFileWasiDeferred)),
      format: 'esm',
      codeSplitting: false,
    },
    transform: {
      target: 'node22',
    },
  });
}

async function bundleThreadedWasiLoaders() {
  fs.rmSync(threadedWasiLoaderArtifactDir, { recursive: true, force: true });
  fs.mkdirSync(threadedWasiLoaderArtifactDir, { recursive: true });

  const loaders: Array<{
    input: string;
    platform: 'browser' | 'node';
    format: 'cjs' | 'esm';
    plugins?: Plugin[];
  }> = [
    {
      input: threadedWasiFiles.binding,
      platform: 'node',
      format: 'cjs',
    },
    {
      input: threadedWasiFiles.browserBinding,
      platform: 'browser',
      format: 'esm',
    },
    {
      input: threadedWasiFiles.worker,
      platform: 'node',
      format: 'esm',
      plugins: [bundleThreadedNodeWorkerRuntime()],
    },
    {
      input: threadedWasiFiles.browserWorker,
      platform: 'browser',
      format: 'esm',
    },
  ];

  for (const { input, platform, format, plugins } of loaders) {
    await build({
      input,
      platform,
      external: platform === 'node' ? [/^node:/, /^@rolldown\/binding-wasm32-wasi(?:\/|$)/] : [],
      output: {
        file: nodePath.join(threadedWasiLoaderArtifactDir, nodePath.basename(input)),
        format,
        codeSplitting: false,
      },
      plugins,
      transform: {
        target: 'node22',
      },
    });
  }
}

function bundleThreadedNodeWorkerRuntime(): Plugin {
  return {
    name: 'bundle-threaded-node-worker-runtime',
    transform: {
      filter: { id: threadedWasiFiles.worker },
      handler(code) {
        // The emnapi-v2 worker template destructures the TSFN/async-work
        // plugins alongside the runtime helpers (the wasm links a "basic"
        // emnapi archive, so every instantiating thread must provide the
        // JavaScript implementations through these plugins).
        const runtimeRequire =
          /const\s*\{\s*instantiateNapiModuleSync,\s*MessageHandler,\s*getDefaultContext,\s*emnapiAsyncWorkPlugin,\s*emnapiTSFNPlugin,?\s*\}\s*=\s*require\(["']@napi-rs\/wasm-runtime["']\);?/;
        if (!runtimeRequire.test(code)) {
          throw new Error('Could not locate the threaded WASI worker runtime require');
        }
        return code.replace(
          runtimeRequire,
          "import { instantiateNapiModuleSync, MessageHandler, getDefaultContext, emnapiAsyncWorkPlugin, emnapiTSFNPlugin } from '@napi-rs/wasm-runtime';",
        );
      },
    },
  };
}

// alias binding file to rolldown-binding.wasi.js and mark it as external
// skip redirection for .d.ts importers so the dts plugin can bundle types
function resolveWasiBinding(isBrowserBuild?: boolean): Plugin {
  return {
    name: 'resolve-wasi-binding',
    resolveId: {
      filter: { id: /\bbinding\b/ },
      async handler(id, importer, options) {
        const resolution = await this.resolve(id, importer, options);

        if (resolution?.id === bindingFile) {
          // Let .d.ts importers resolve normally so binding types get bundled inline
          if (importer && /\.d\.[cm]?ts$/.test(importer)) return resolution;
          const id = isBrowserBuild ? bindingFileWasiBrowser : bindingFileWasi;
          return { id, external: 'relative' };
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
      filter: { id: /^node:/ },
      handler(id, importer) {
        if (id === 'node:path') {
          return this.resolve('pathe');
        }
        if (
          id === 'node:os' ||
          id === 'node:async_hooks' ||
          id === 'node:worker_threads' ||
          id === 'node:url' ||
          id === 'node:fs/promises' ||
          id === 'node:fs' ||
          id === 'node:util'
        ) {
          // conditional import
          return { id, external: true, moduleSideEffects: false };
        }
        throw new Error(`Unresolved module: ${id} from ${importer}`);
      },
    },
  };
}

function patchBindingJs(): Plugin {
  return {
    name: 'patch-binding-js',
    transform: {
      filter: {
        id: 'src/binding.cjs',
      },
      handler(code) {
        return (
          code
            // inject binding auto download fallback for webcontainer
            .replace(
              '\nif (!nativeBinding) {',
              (s) =>
                `
if (!nativeBinding && globalThis.process?.versions?.["webcontainer"]) {
  try {
    nativeBinding = require('./webcontainer-fallback.cjs');
    loadedBindingTarget =
      nativeBinding.__rolldownBindingTarget === 'wasi' ? 'wasi' : 'wasi-threads';
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

function generateRuntimeTypes() {
  const inputFile = nodePath.resolve(
    __dirname,
    '../../crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js',
  );
  const outputFile = nodePath.resolve(buildMeta.buildOutputDir, 'experimental-runtime-types.d.ts');

  console.log(styleText('green', '[build:done]'), 'Generating dts from', inputFile);

  const jsCode = fs.readFileSync(inputFile, 'utf-8');
  const result = ts.transpileDeclaration(jsCode, {
    compilerOptions: {
      ...getTsconfigCompilerOptionsForFile(inputFile),
      noEmit: false,
      emitDeclarationOnly: true,
    },
    fileName: inputFile,
  });

  if (result && result.outputText) {
    fs.writeFileSync(outputFile, result.outputText, 'utf-8');
  } else {
    throw new Error('Failed to generate d.ts from runtime-extra-dev.js');
  }
}

function getTsconfigCompilerOptionsForFile(file: string) {
  const tsconfigPath = ts.findConfigFile(file, (path) => ts.sys.fileExists(path));
  let compilerOptions = ts.getDefaultCompilerOptions();
  if (tsconfigPath) {
    const parsedConfig = ts.getParsedCommandLineOfConfigFile(tsconfigPath, undefined, {
      ...ts.sys,
      onUnRecoverableConfigFileDiagnostic(diag) {
        console.error(diag);
      },
    });
    if (!parsedConfig) throw new Error();
    if (parsedConfig.errors.length > 0) {
      throw new AggregateError(parsedConfig.errors);
    }
    compilerOptions = parsedConfig.options;
  }
  return compilerOptions;
}

/**
 * Removes {@include ...} tags from generated .d.ts files.
 * These tags are only used for the docs site and should not appear in the published types.
 */
function removeIncludeTagsFromDts(): Plugin {
  const includeTagRegex = /\s*\{@include\s+[^}]+\}/g;

  return {
    name: 'remove-include-tags-from-dts',
    generateBundle(_options, bundle) {
      for (const [fileName, output] of Object.entries(bundle)) {
        if (!fileName.endsWith('.d.ts') && !fileName.endsWith('.d.mts')) {
          continue;
        }
        if (output.type === 'asset') {
          this.warn(
            `Expected .d.ts files to be chunks, but found asset type for ${fileName} (type: ${output.type}).`,
          );
        } else if (output.type === 'chunk') {
          const matches = output.code.match(includeTagRegex);
          if (matches) {
            output.code = output.code.replace(includeTagRegex, '');
          }
        }
      }
    },
  };
}
