import fs from 'node:fs';
import nodePath from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

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
  // dists: the artifact sets have distinct per-flavor names, so each target
  // wires its dist to its own flavor's loaders.
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
const bindingFileWasi = nodePath.resolve(
  buildMeta.wasmSingleThread ? 'src/rolldown-binding.wasip1.cjs' : 'src/rolldown-binding.wasi.cjs',
);
const bindingFileWasiBrowser = nodePath.resolve(
  buildMeta.wasmSingleThread
    ? 'src/rolldown-binding.wasip1-browser.js'
    : 'src/rolldown-binding.wasi-browser.js',
);
const threadedWasiLoaderArtifactDir = nodePath.resolve('artifacts/threaded-wasi-loaders');
const threadedWasiFiles = {
  binding: nodePath.resolve('src/rolldown-binding.wasi.cjs'),
  browserBinding: nodePath.resolve('src/rolldown-binding.wasi-browser.js'),
  worker: nodePath.resolve('src/wasi-worker.mjs'),
  browserWorker: nodePath.resolve('src/wasi-worker-browser.mjs'),
};
const commonRuntimeInputFile = nodePath.resolve(
  __dirname,
  '../../crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js',
);
const runtimeBaseInputFile = nodePath.resolve(
  __dirname,
  '../../crates/rolldown/src/runtime/runtime-base.js',
);
const defaultRuntimeInputFile = nodePath.resolve(
  __dirname,
  '../../crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-default.js',
);

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
  await buildRuntimeEntry();
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
        __RUNTIME_STRING__: JSON.stringify(readDefaultDevRuntimeSource()),
      },
    },
  };
}

// Keep the managed workerd entries self-contained so release staging can reuse
// the same public factory across every package that ships them.
// See internal-docs/workerd-managed-instance/implementation.md.
async function bundleManagedWorkerdLoaders() {
  // The workerd entries drive the browser pipeline against per-instance managed
  // bindings, so from the pipeline's point of view they are browser builds.
  const workerdDefine = {
    'import.meta.browserBuild': 'true',
    'import.meta.workerdPackageApi': 'true',
    // Same polyfill as the browser package build: workerd has no ambient
    // `process`, and the logger init reads this flag.
    'process.env.ROLLDOWN_TEST': 'false',
    // Same as the browser package build: the default dev runtime source must be
    // embedded, because workerd has no filesystem fallback to read it from.
    __RUNTIME_STRING__: JSON.stringify(readDefaultDevRuntimeSource()),
  };

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
    plugins: [aliasWorkerdPipelineModules()],
    transform: {
      target: 'node22',
      define: workerdDefine,
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
    plugins: [aliasWorkerdPipelineModules(), removeBuiltModules()],
    transform: {
      target: 'node22',
      define: workerdDefine,
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

// Published consumers do not inherit the workspace `overrides` pin, so bundle
// the generated loaders last: every package-root condition must embed the exact
// emnapi runtime the release was built with.
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
        // The cli worker template destructures the two emnapi plugins next to
        // the runtime helpers and passes them to `instantiateNapiModuleSync`,
        // so the hoisted import binds all five names. The threaded wasm links
        // the full emnapi archive and imports no async-work or
        // threadsafe-function functions; the plugin names are bound only
        // because the template uses them.
        const runtimeRequire =
          /const\s*\{\s*instantiateNapiModuleSync,\s*MessageHandler,\s*getDefaultContext,\s*emnapiAsyncWorkPlugin,\s*emnapiTSFNPlugin,?\s*\}\s*=\s*require\(["']@napi-rs\/wasm-runtime["']\);?/;
        if (!runtimeRequire.test(code)) {
          throw new Error('Could not locate the threaded WASI worker runtime require');
        }
        // The template makes this require inside a `try` block (a failure
        // there must raise the crash flags), where an `import` cannot go.
        // Hoist the import to the top level and bind the same names in place.
        const names = [
          'instantiateNapiModuleSync',
          'MessageHandler',
          'getDefaultContext',
          'emnapiAsyncWorkPlugin',
          'emnapiTSFNPlugin',
        ];
        const runtimeImport = `import { ${names
          .map((name) => `${name} as __wasmRuntime_${name}`)
          .join(', ')} } from '@napi-rs/wasm-runtime';\n`;
        const runtimeBindings = `const ${names
          .map((name) => `${name} = __wasmRuntime_${name}`)
          .join(', ')};`;
        return runtimeImport + code.replace(runtimeRequire, runtimeBindings);
      },
    },
  };
}

// Binding exports the workerd proxy must not forward: the seven private
// CurrentThread host exports (the deferred loader registers them; a pipeline
// import of one must fail the bundle), and `getRuntimeCapabilities`,
// which the proxy defines itself.
const WORKERD_PROXY_SKIPPED_EXPORTS: ReadonlySet<string> = new Set([
  'getCurrentThreadTaskHostContractVersion',
  'isCurrentThreadHostRegistrationActive',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
  'reserveCurrentThreadHostRegistration',
  'unregisterCurrentThreadTaskHost',
  'unregisterTimerHost',
  'getRuntimeCapabilities',
]);

// The names the cli wrote into the `napi-rs-artifact-metadata` header on the
// first line of the threadless loader.
function readWasip1ArtifactExports(): string[] {
  const loaderPath = nodePath.resolve('src/rolldown-binding.wasip1.cjs');
  const firstLine = fs.readFileSync(loaderPath, 'utf8').split('\n', 1)[0];
  const marker = 'napi-rs-artifact-metadata:';
  const markerIndex = firstLine.indexOf(marker);
  if (markerIndex === -1) {
    throw new Error(`${loaderPath} has no ${marker} header on its first line`);
  }
  const { exports } = JSON.parse(firstLine.slice(markerIndex + marker.length)) as {
    exports?: unknown;
  };
  if (!Array.isArray(exports) || exports.some((name) => typeof name !== 'string')) {
    throw new Error(`The ${marker} header of ${loaderPath} has no string \`exports\` list`);
  }
  return exports;
}

// The bundled workerd entries must never evaluate ambient-binding side
// effects; the binding is entered per managed instance instead.
// - `src/binding.cjs` -> forwarding proxy (`binding-workerd-proxy`); the
//   `transform` hook appends one `lazyExport` per artifact export, marked pure
//   so the unused ones tree-shake away
// - `src/binding-magic-string.ts` -> throwing stub (prototype mutation at
//   module evaluation needs a live binding)
// The dts-only workerd build omits this plugin so declarations keep coming
// from the real modules.
function aliasWorkerdPipelineModules(): Plugin {
  const proxyFile = nodePath.resolve('src/binding-workerd-proxy.ts');
  const aliases = new Map([
    [bindingFile, proxyFile],
    [
      nodePath.resolve('src/binding-magic-string.ts'),
      nodePath.resolve('src/workerd-stubs/binding-magic-string.ts'),
    ],
  ]);
  return {
    name: 'alias-workerd-pipeline-modules',
    resolveId: {
      filter: { id: /binding/ },
      async handler(id, importer, options) {
        const resolution = await this.resolve(id, importer, options);
        if (!resolution) return resolution;
        const target = aliases.get(resolution.id);
        if (target) return { id: target };
        return resolution;
      },
    },
    transform: {
      filter: { id: /binding-workerd-proxy\.ts$/ },
      handler(code, id) {
        if (id !== proxyFile) return;
        const lazyExports = readWasip1ArtifactExports()
          .filter((name) => !WORKERD_PROXY_SKIPPED_EXPORTS.has(name))
          .map(
            (name) => `export const ${name} = /* @__PURE__ */ lazyExport(${JSON.stringify(name)});`,
          );
        return `${code}\n${lazyExports.join('\n')}\n`;
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
    // The fallback loads a generated WASI loader, which reports its own
    // flavor; adopt it the same way upstream's NAPI_RS_NATIVE_LIBRARY_PATH
    // branch does, or this loader would misreport the artifact as native.
    __napiLoadedBindingTarget =
      typeof nativeBinding.__napiBindingTarget === 'string'
        ? nativeBinding.__napiBindingTarget
        : 'native';
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

// Vite serves this file to the browser as is, so it must have no imports.
// See internal-docs/dev-engine/implementation.md
async function buildRuntimeEntry() {
  const helperNames = Object.keys(await import(pathToFileURL(runtimeBaseInputFile).href));
  await build({
    input: { 'experimental-runtime': commonRuntimeInputFile },
    platform: 'neutral',
    transform: {
      inject: Object.fromEntries(helperNames.map((name) => [name, [runtimeBaseInputFile, name]])),
    },
    output: {
      dir: buildMeta.buildOutputDir,
      format: 'esm',
      entryFileNames: '[name].mjs',
    },
  });
}

function generateRuntimeTypes() {
  const outputFile = nodePath.resolve(buildMeta.buildOutputDir, 'experimental-runtime.d.ts');

  console.log(styleText('green', '[build:done]'), 'Generating dts from', commonRuntimeInputFile);

  const commonRuntimeSource = fs.readFileSync(commonRuntimeInputFile, 'utf-8');
  const result = ts.transpileDeclaration(commonRuntimeSource, {
    compilerOptions: {
      ...getTsconfigCompilerOptionsForFile(commonRuntimeInputFile),
      noEmit: false,
      emitDeclarationOnly: true,
    },
    fileName: commonRuntimeInputFile,
  });

  if (result && result.outputText) {
    fs.writeFileSync(outputFile, result.outputText, 'utf-8');
    fs.copyFileSync(
      outputFile,
      nodePath.resolve(buildMeta.buildOutputDir, 'experimental-runtime-types.d.ts'),
    );
  } else {
    throw new Error('Failed to generate d.ts from runtime-extra-dev.js');
  }
}

function readDefaultDevRuntimeSource() {
  const commonRuntimeSource = fs.readFileSync(commonRuntimeInputFile, 'utf-8');
  const defaultRuntimeSource = fs.readFileSync(defaultRuntimeInputFile, 'utf-8');
  return `${commonRuntimeSource}\n${defaultRuntimeSource}`;
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
