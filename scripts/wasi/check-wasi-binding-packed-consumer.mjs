import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import {
  cp,
  copyFile,
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

import { parse } from 'acorn';
import { chromium } from 'playwright-chromium';

import { findBareRuntimeImports } from './bare-runtime-imports.mjs';

const execFileAsync = promisify(execFile);
const repoRoot = fileURLToPath(new URL('../../', import.meta.url));
const rootPackageDir = path.join(repoRoot, 'packages/rolldown');
const browserPackageDir = path.join(repoRoot, 'packages/browser');
const napiCli = path.join(rootPackageDir, 'node_modules/@napi-rs/cli/dist/cli.js');
const typescriptCli = createRequire(import.meta.url).resolve('typescript/bin/tsc');
const rootPackageManifest = JSON.parse(
  await readFile(path.join(rootPackageDir, 'package.json'), 'utf8'),
);
const publicTypesVersion = JSON.parse(
  await readFile(path.join(rootPackageDir, 'node_modules/@oxc-project/types/package.json'), 'utf8'),
).version;
const runtimePackages = ['@emnapi/core', '@emnapi/runtime', '@napi-rs/wasm-runtime', 'buffer'];
// The loaders are generated from these two manifest fields, so read them here
// rather than restating the numbers (a hardcoded copy silently goes stale).
const threadlessInitialPages = rootPackageManifest.napi.wasm.threadlessInitialMemory;
const threadlessMaximumPages = rootPackageManifest.napi.wasm.maximumMemory;
const wasmPageBytes = 64 * 1024;
const flavors = [
  {
    key: 'threaded',
    dir: path.join(repoRoot, 'packages/rolldown/npm/wasm32-wasi'),
    target: 'wasm32-wasip1-threads',
    loaders: [
      { name: 'rolldown-binding.wasi.cjs', sourceType: 'script' },
      { name: 'rolldown-binding.wasi-browser.js', sourceType: 'module' },
      { name: 'wasi-worker.mjs', sourceType: 'module' },
      { name: 'wasi-worker-browser.mjs', sourceType: 'module' },
    ],
    files: [
      'rolldown-binding.wasm32-wasi.wasm',
      'rolldown-binding.wasi.cjs',
      'rolldown-binding.wasi.d.cts',
      'rolldown-binding.wasi-browser.js',
      'wasi-worker.mjs',
      'wasi-worker-browser.mjs',
      'LICENSE',
      'THIRD-PARTY-LICENSE',
    ],
  },
  {
    key: 'threadless',
    dir: path.join(repoRoot, 'packages/rolldown/npm/wasm32-wasip1'),
    target: 'wasm32-wasip1',
    loaders: [
      { name: 'rolldown-binding.wasip1.cjs', sourceType: 'script' },
      { name: 'rolldown-binding.wasip1-browser.js', sourceType: 'module' },
      { name: 'rolldown-binding.wasip1-deferred.js', sourceType: 'module' },
    ],
    files: [
      'rolldown-binding.wasm32-wasip1.wasm',
      'rolldown-binding.wasip1.cjs',
      'rolldown-binding.wasip1.d.cts',
      'rolldown-binding.wasip1-browser.js',
      'rolldown-binding.wasip1-deferred.js',
      'rolldown-binding.wasip1-deferred.d.ts',
      'rolldown-binding.wasm32-wasip1.wasm.d.ts',
      'LICENSE',
      'THIRD-PARTY-LICENSE',
    ],
  },
];
const requiredNotices = {
  LICENSE: ['VoidZero Inc.'],
  'THIRD-PARTY-LICENSE': [
    '@emnapi/core',
    '@emnapi/runtime',
    '@emnapi/wasi-threads',
    '@napi-rs/wasm-runtime',
    '@tybys/wasm-util',
    'tslib',
    'base64-js',
    'Copyright (c) 2014 Jameson Little',
    'buffer is distributed under the MIT license',
    'Copyright (c) Feross Aboukhadijeh, and other contributors.',
    'ieee754',
    'Copyright 2008 Fair Oaks Labs, Inc.',
    'safe-buffer',
    'Copyright (c) Feross Aboukhadijeh',
    'checked during WASI package staging',
    '@jsonjoy.com/fs-core',
    'abort-controller',
    'glob-to-regex.js',
    'punycode',
    'qs',
    'Apache License',
  ],
};
const generatedRootFiles = {
  workerdEntry: 'rolldown-binding.wasm32-wasip1.workerd.mjs',
  workerdTypeDef: 'rolldown-binding.wasm32-wasip1.workerd.d.mts',
  wasmEntry: 'rolldown-binding.wasm32-wasip1.wasm',
  wasmTypeDef: 'rolldown-binding.wasm32-wasip1.wasm.d.mts',
};
const internalRuntimeExports = [
  'EnhancedTransformTask',
  'IsolatedDeclarationTask',
  'MinifyTask',
  'ModuleRunnerTransformTask',
  'ResolveDtsTask',
  'ResolveFileTask',
  'ResolveTask',
  'TransformTask',
];
const consumerTimeoutMs = 120_000;
const browserExerciseTimeoutMs = 30_000;
const pnpmVersion = '11.9.0';
const yarnVersion = '1.22.22';
const tempDir = await mkdtemp(path.join(tmpdir(), 'rolldown-wasi-packed-consumer-'));
let runtimeNodes = [];

async function run(command, args, options = {}) {
  return execFileAsync(command, args, {
    maxBuffer: 20 * 1024 * 1024,
    timeout: consumerTimeoutMs,
    ...options,
    env: {
      ...process.env,
      COREPACK_ENABLE_DOWNLOAD_PROMPT: '0',
      ...options.env,
    },
  });
}

async function resolveRuntimeNodes() {
  const currentVersion = process.versions.node;
  const [currentMajor, currentMinor] = currentVersion.split('.').map(Number);
  const nodes = [{ executable: process.execPath, version: `v${currentVersion}` }];
  if (currentMajor === 20 && currentMinor >= 19) return nodes;

  const node20Executable = process.env.ROLLDOWN_NODE20_EXECUTABLE?.trim();
  assert.ok(
    node20Executable,
    'Set ROLLDOWN_NODE20_EXECUTABLE to a Node 20.19+ binary for packed consumer validation',
  );
  const { stdout } = await run(node20Executable, ['--version']);
  const version = stdout.trim();
  const match = /^v(\d+)\.(\d+)\.\d+/.exec(version);
  assert.ok(match, `Unable to parse Node 20 runtime version: ${JSON.stringify(version)}`);
  assert.equal(Number(match[1]), 20, `Expected a Node 20 runtime, received ${version}`);
  assert.ok(Number(match[2]) >= 19, `Expected Node 20.19 or newer, received ${version}`);
  nodes.push({ executable: node20Executable, version });
  return nodes;
}

function runPnpm(args, options) {
  return run('corepack', [`pnpm@${pnpmVersion}`, ...args], options);
}

function runYarn(args, options) {
  return run('corepack', [`yarn@${yarnVersion}`, ...args], {
    ...options,
    env: {
      YARN_CACHE_FOLDER: path.join(tempDir, 'yarn-cache'),
      ...options?.env,
    },
  });
}

async function pack(packageDir, packDir) {
  await mkdir(packDir, { recursive: true });
  const before = new Set(await readdir(packDir));
  await runPnpm(['--pm-on-fail=ignore', 'pack', '--pack-destination', packDir], {
    cwd: packageDir,
  });
  const tarballs = (await readdir(packDir)).filter(
    (entry) => entry.endsWith('.tgz') && !before.has(entry),
  );
  assert.equal(tarballs.length, 1, `Expected one tarball from ${packageDir}, found ${tarballs}`);
  return path.join(packDir, tarballs[0]);
}

async function runNodeModule(cwd, filename, source, args, options = {}) {
  const entry = path.join(cwd, filename);
  await writeFile(entry, source);
  const { compareStdout = true, nodeArgs = [], returnAllResults = false, ...runOptions } = options;
  let primaryResult;
  const results = [];
  for (const runtimeNode of runtimeNodes) {
    const result = await run(runtimeNode.executable, [...nodeArgs, entry, ...args], {
      ...runOptions,
      cwd,
    });
    results.push({ ...result, runtimeVersion: runtimeNode.version });
    if (compareStdout && primaryResult) {
      assert.equal(
        result.stdout,
        primaryResult.stdout,
        `${filename} output differs under ${runtimeNode.version}`,
      );
    } else {
      primaryResult = result;
    }
  }
  return returnAllResults ? results : primaryResult;
}

async function typecheckPackedConsumer(cwd, filename, source) {
  const entry = path.join(cwd, filename);
  await writeFile(entry, source);
  await run(
    process.execPath,
    [
      typescriptCli,
      '--noEmit',
      '--strict',
      '--module',
      'NodeNext',
      '--moduleResolution',
      'NodeNext',
      '--target',
      'ES2022',
      '--lib',
      'ES2022,DOM,ESNext.Disposable',
      '--verbatimModuleSyntax',
      entry,
    ],
    { cwd },
  );
}

async function withTimeout(promise, timeoutMs, label) {
  let timeout;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timeout = setTimeout(
          () => reject(new Error(`${label} timed out after ${timeoutMs}ms`)),
          timeoutMs,
        );
      }),
    ]);
  } finally {
    clearTimeout(timeout);
  }
}

function fileDependency(fromDir, tarball) {
  return `file:${path.relative(fromDir, tarball).split(path.sep).join('/')}`;
}

function getInstalledOptionalPackageDirs(consumerDir, packedFlavors) {
  const consumerRequire = createRequire(path.join(consumerDir, 'package.json'));
  const rootRequire = createRequire(consumerRequire.resolve('rolldown/package.json'));
  return new Map(
    [...packedFlavors.values()].map((flavor) => [
      flavor.key,
      path.dirname(rootRequire.resolve(`${flavor.name}/package.json`)),
    ]),
  );
}

async function withOnlyOptionalFlavor(packageDirs, selectedKey, callback) {
  const disabledPackages = [];
  try {
    for (const [key, packageDir] of packageDirs) {
      if (key === selectedKey) continue;
      const disabledDir = `${packageDir}.disabled-for-${selectedKey}-validation`;
      await rename(packageDir, disabledDir);
      disabledPackages.push([packageDir, disabledDir]);
    }
    for (const [key, packageDir] of packageDirs) {
      assert.equal(
        existsSync(packageDir),
        key === selectedKey,
        `Expected only the ${selectedKey} optional package to remain installed`,
      );
    }
    return await callback();
  } finally {
    for (const [packageDir, disabledDir] of disabledPackages.reverse()) {
      await rename(disabledDir, packageDir);
    }
  }
}

function assertNoRegistryRuntimePackages(lockfile, packageManager) {
  assert.doesNotMatch(
    lockfile,
    /@emnapi\//,
    `${packageManager} must not install any registry @emnapi package`,
  );
  assert.doesNotMatch(
    lockfile,
    /@napi-rs\/wasm-runtime/,
    `${packageManager} must not install registry @napi-rs/wasm-runtime`,
  );
  assert.doesNotMatch(
    lockfile,
    /(?:^|\n)(?: {2})?["']?buffer@|["']node_modules\/buffer["']\s*:/,
    `${packageManager} must not install registry buffer`,
  );
}

const threadlessCapabilities = {
  backend: 'shared',
  flavor: 'CurrentThread',
  target: 'wasi',
  wasi: true,
  asyncRuntimeBuild: true,
  threads: false,
  timers: true,
  devSupported: false,
  watchSupported: false,
  blockOnJsThreadSafe: false,
};

const threadedCapabilities = {
  backend: 'shared',
  flavor: 'CurrentThread',
  target: 'wasi-threads',
  wasi: true,
  asyncRuntimeBuild: true,
  threads: false,
  timers: true,
  devSupported: false,
  watchSupported: false,
  blockOnJsThreadSafe: false,
};

function assertThreadlessExercise(stdout) {
  const result = JSON.parse(stdout);
  const runtimeExports = result.runtimeExports;
  const workerdExports = result.workerdExports;
  const resolution = result.resolution;
  delete result.runtimeExports;
  delete result.workerdExports;
  delete result.resolution;
  assert.deepEqual(result, {
    grew: true,
    outputs: 1,
    capabilities: threadlessCapabilities,
  });
  assert.ok(Array.isArray(runtimeExports));
  assert.ok(Array.isArray(workerdExports));
  for (const privateExport of [
    'getDeferredInstanceBinding',
    'cancelCurrentThreadRuntimeTaskDispatch',
    'driveCurrentThreadRuntimeTasks',
    'registerCurrentThreadTaskHost',
    'registerTimerHost',
    'unregisterCurrentThreadTaskHost',
    'unregisterTimerHost',
  ]) {
    assert.equal(
      workerdExports.includes(privateExport),
      false,
      `Managed workerd package entry must not expose ${privateExport}`,
    );
    assert.equal(
      runtimeExports.includes(privateExport),
      false,
      `Managed workerd binding facade must not expose ${privateExport}`,
    );
  }
  assert.equal(typeof resolution?.workerd, 'string');
  assert.equal(typeof resolution?.wasm, 'string');
  return { runtimeExports, workerdExports, resolution };
}

function assertThreadlessBindingExercise(stdout) {
  assert.deepEqual(JSON.parse(stdout), {
    outputs: 1,
    capabilities: threadlessCapabilities,
    taskHostContractVersion: 4,
  });
}

function assertThreadlessBrowserBindingExercise(stdout) {
  const result = JSON.parse(stdout);
  const resolved = result.resolved;
  delete result.resolved;
  assert.deepEqual(result, {
    outputs: 1,
    capabilities: threadlessCapabilities,
    taskHostContractVersion: 4,
  });
  assert.ok(
    resolved.endsWith('/rolldown-binding.wasip1-browser.js'),
    `Browser condition resolved threadless binding to ${resolved}`,
  );
}

function assertRootPackageExercise(stdout, flavor) {
  const threadless = flavor === 'threadless';
  assert.deepEqual(JSON.parse(stdout), {
    outputs: 1,
    capabilities: threadless ? threadlessCapabilities : threadedCapabilities,
    support: {
      dev: false,
      watch: false,
      dynamicImportVarsResolver: true,
      importGlobResolver: true,
      parallelPlugins: false,
      pluginErrorMetadata: true,
      symlinks: false,
      threadlessWasi: threadless,
      workerd: false,
    },
  });
}

// @napi-rs/cli 3.10.0 tore the threaded WASI pool down with a bare
// `worker.terminate()`, which @emnapi/wasi-threads never recorded as an
// expected termination: its worker exit listener rethrew `Worker stopped with
// exit code 1` and killed the consumer right after
// `Symbol.for('napi.rs.wasi.dispose')` settled. 3.10.1 routes the termination
// through the captured thread manager, so a real build followed by a disposal
// has to stay clean on every runtime we support.
function assertThreadedDisposeExercise(results) {
  for (const { stdout, runtimeVersion } of results) {
    // The exercise unrefs its own watchdog, so the loader alone holds the event
    // loop open while the disposal is in flight. A loader that stops doing that
    // lets Node exit mid-disposal: the child ends successfully having written
    // nothing, which is exactly what this first assertion catches.
    assert.ok(
      stdout.includes('"marker":"threaded-wasi-dispose"'),
      `The threaded WASI dispose exercise exited under ${runtimeVersion} without reporting a result (stdout: ${JSON.stringify(stdout)}), which is how a disposal that no longer holds the event loop open shows up`,
    );
    const result = JSON.parse(stdout);
    const workers = result.workers;
    const exitedWorkers = result.exitedWorkers;
    delete result.workers;
    delete result.exitedWorkers;
    assert.deepEqual(
      result,
      {
        marker: 'threaded-wasi-dispose',
        target: 'wasi-threads',
        outputs: 1,
        disposed: true,
        workerErrors: [],
      },
      `Disposing the threaded WASI binding after a real build failed under ${runtimeVersion}`,
    );
    assert.ok(
      workers >= 1,
      `The threaded WASI build spawned no pool worker under ${runtimeVersion}, so the disposal never exercised worker termination`,
    );
    // A settled disposal promise proves nothing on its own: a loader that skips
    // termination outright settles just as fast. Only the workers' own `exit`
    // events show the pool actually went away.
    assert.equal(
      exitedWorkers,
      workers,
      `Only ${exitedWorkers} of ${workers} threaded WASI pool workers exited under ${runtimeVersion} after the disposal settled`,
    );
  }
}

// Counterpart of `renderDisposeInFlightExerciseSource`. Every property here
// fails on a loader that tears the environment down over outstanding
// `napi_async_work`, and each one fails differently, so the report says which:
//
//   - a missing marker means the child exited mid-disposal (no keep-alive),
//   - `unsettled` async work means the completion callbacks were stranded,
//   - a worker that never emitted `exit` means the pool outlived the disposal.
function assertDisposeInFlightExercise(results, { marker, target, threaded }) {
  for (const { stdout, runtimeVersion } of results) {
    assert.ok(
      stdout.includes(`"marker":"${marker}"`),
      `The ${marker} exercise exited under ${runtimeVersion} without reporting a result (stdout: ${JSON.stringify(stdout)}), which is how a disposal that no longer holds the event loop open shows up`,
    );
    const result = JSON.parse(stdout);
    assert.equal(
      result.target,
      target,
      `The ${marker} exercise ran against ${result.target} under ${runtimeVersion}`,
    );
    assert.equal(
      result.disposed,
      true,
      `Disposing over in-flight work never settled under ${runtimeVersion}`,
    );
    assert.deepEqual(
      result.failures,
      [],
      `Disposing over in-flight work raised out-of-band failures under ${runtimeVersion}`,
    );
    assert.deepEqual(
      result.workerErrors,
      [],
      `A WASI pool worker errored while disposing over in-flight work under ${runtimeVersion}`,
    );
    // The build is the realistic shape, not the drain's subject: it may finish
    // or be cancelled depending on how far it got, and both are a settled
    // promise. Only never settling is a defect.
    assert.ok(
      result.build === 'fulfilled' || result.build.startsWith('rejected:'),
      `The bundle build disposed over never settled under ${runtimeVersion}: ${result.build}`,
    );
    // This is the drain's subject. Work already executing refuses cancellation
    // and finishes normally; work still queued is cancelled and surfaces as the
    // `AbortError` napi-rs maps `napi_cancelled` to. Which of the two a given
    // task lands on depends on the pool, so accept either — but nothing else,
    // and never `unsettled`.
    assert.equal(
      result.asyncWork.length,
      8,
      `The ${marker} exercise reported ${result.asyncWork.length} async-work outcomes under ${runtimeVersion}`,
    );
    for (const outcome of result.asyncWork) {
      assert.ok(
        outcome === 'fulfilled' || outcome.startsWith('rejected:AbortError:'),
        `Async work outstanding at disposal settled as ${JSON.stringify(outcome)} under ${runtimeVersion}; stranded work reports "unsettled"`,
      );
    }
    if (threaded) {
      assert.ok(
        result.workers >= 1,
        `The threaded WASI build spawned no pool worker under ${runtimeVersion}, so the disposal never exercised worker termination`,
      );
      assert.equal(
        result.exitedWorkers,
        result.workers,
        `Only ${result.exitedWorkers} of ${result.workers} threaded WASI pool workers exited under ${runtimeVersion} after disposing over in-flight work`,
      );
    } else {
      assert.equal(
        result.workers,
        0,
        `The threadless WASI binding spawned ${result.workers} pool workers under ${runtimeVersion}`,
      );
    }
  }
}

function assertBrowserPackageExercise(stdout) {
  const result = JSON.parse(stdout);
  const resolution = result.resolution;
  delete result.resolution;
  assert.deepEqual(result, {
    outputs: 1,
    capabilities: {
      backend: 'shared',
      flavor: 'CurrentThread',
      target: 'wasi',
      wasi: true,
      asyncRuntimeBuild: true,
      threads: false,
      timers: true,
      devSupported: false,
      watchSupported: false,
      blockOnJsThreadSafe: false,
    },
    support: {
      dev: false,
      watch: false,
      dynamicImportVarsResolver: true,
      importGlobResolver: true,
      parallelPlugins: false,
      pluginErrorMetadata: true,
      symlinks: false,
      threadlessWasi: true,
      workerd: true,
    },
  });
  assert.ok(
    resolution.root.endsWith('/dist/index.browser.mjs'),
    `Browser condition resolved package root to ${resolution.root}`,
  );
  assert.ok(
    resolution.experimental.endsWith('/dist/experimental-index.browser.mjs'),
    `Browser condition resolved experimental API to ${resolution.experimental}`,
  );
}

function assertBrowserPackageChromiumExercise(result) {
  assert.deepEqual(
    {
      callbackCalls: result.callbackCalls,
      initialSupport: result.initialSupport,
      outputs: result.outputs,
      pluginError: result.pluginError,
      providerStoreAfterAwait: result.providerStoreAfterAwait,
      reentrantError: result.reentrantError,
      support: result.support,
      transform: result.transform,
    },
    {
      callbackCalls: {
        buildStart: 1,
        load: 1,
        resolveId: 1,
      },
      initialSupport: {
        source: 'unavailable',
        supported: false,
      },
      outputs: 1,
      pluginError: {
        causeIdentity: true,
        code: 'PLUGIN_ERROR',
        customMarker: 'browser-retained',
        hook: 'transform',
        id: 'virtual:metadata',
        identity: true,
        nestedMarker: 37,
        plugin: 'packed-browser-metadata',
        pluginCode: 'BROWSER_USER_CODE',
        stackRetained: true,
      },
      providerStoreAfterAwait: 'propagated',
      reentrantError:
        "Cannot call bundle.generate() or bundle.write() from one of the same bundle's active JavaScript callbacks",
      support: {
        source: 'custom',
        supported: true,
      },
      transform: {
        code: 'const answer = 42;\n',
        errors: 0,
        warnings: 0,
      },
    },
  );
  assert.match(
    result.unavailableError.message,
    /browser require async-context propagation.*configureAsyncContext/s,
  );
  assert.deepEqual(
    {
      code: result.unavailableError.code,
      name: result.unavailableError.name,
    },
    {
      code: 'ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE',
      name: 'AsyncContextUnavailableError',
    },
  );
  assert.ok(
    result.storageCreations >= 2,
    `Expected browser provider storage for close and build contexts, received ${result.storageCreations}`,
  );
  assert.ok(result.storageRuns > 0, 'Expected the configured browser provider to run callbacks');
}

function assertResolutionSuffix(resolution, key, suffix) {
  assert.ok(
    resolution[key].endsWith(suffix),
    `${key} resolved to ${resolution[key]}, expected suffix ${suffix}`,
  );
}

async function assertPackedNotices(packageDir, manifest, packageName) {
  for (const [notice, expectedContents] of Object.entries(requiredNotices)) {
    assert.ok(
      manifest.files?.includes(notice),
      `${notice} must be explicit in the ${packageName} packlist`,
    );
    const contents = await readFile(path.join(packageDir, notice), 'utf8');
    for (const expectedContent of expectedContents) {
      assert.ok(
        contents.includes(expectedContent),
        `${packageName} ${notice} must attribute ${expectedContent}`,
      );
    }
  }
}

function collectBindingNames(pattern, names) {
  if (!pattern) return;
  if (pattern.type === 'Identifier') {
    names.add(pattern.name);
    return;
  }
  if (pattern.type === 'ObjectPattern') {
    for (const property of pattern.properties) {
      collectBindingNames(
        property.type === 'RestElement' ? property.argument : property.value,
        names,
      );
    }
    return;
  }
  if (pattern.type === 'ArrayPattern') {
    for (const element of pattern.elements) collectBindingNames(element, names);
    return;
  }
  if (pattern.type === 'AssignmentPattern' || pattern.type === 'RestElement') {
    collectBindingNames(pattern.left ?? pattern.argument, names);
  }
}

function findModuleExports(code) {
  const program = parse(code, { ecmaVersion: 'latest', sourceType: 'module' });
  const exports = new Set();
  for (const statement of program.body) {
    if (statement.type === 'ExportDefaultDeclaration') {
      exports.add('default');
      continue;
    }
    if (statement.type !== 'ExportNamedDeclaration') continue;
    for (const specifier of statement.specifiers) {
      exports.add(specifier.exported.name ?? specifier.exported.value);
    }
    if (statement.declaration?.type === 'VariableDeclaration') {
      for (const declaration of statement.declaration.declarations) {
        collectBindingNames(declaration.id, exports);
      }
    } else if (statement.declaration?.id?.name) {
      exports.add(statement.declaration.id.name);
    }
  }
  return [...exports].sort((a, b) => a.localeCompare(b));
}

// The dts bundler emits the workerd entry's own exports inline (`export declare
// function createInstance(...)`) and only re-exports types through a trailing
// list, so there is no single `export { ... }` block to read. Collect both
// shapes, anchored at column 0 so the indented blocks inside the bundled
// `declare namespace` wrappers stay out of the top-level export set.
function collectDeclarationExports(declaration) {
  const names = new Set();
  const declarationPattern =
    /^export\s+(?:declare\s+)?(?:abstract\s+)?(?:const|let|var|function|class|interface|type|enum|namespace)\s+([A-Za-z_$][\w$]*)/gm;
  for (const [, name] of declaration.matchAll(declarationPattern)) names.add(name);
  const listPattern = /^export\s+(?:type\s+)?\{([^}]*)\}/gm;
  for (const [, list] of declaration.matchAll(listPattern)) {
    for (const entry of list.split(',')) {
      const name = entry.trim();
      if (!name) continue;
      names.add(name.split(/\s+as\s+/).at(-1));
    }
  }
  return names;
}

async function assertWorkerdDeclarationParity(packageDir, runtimeExports, workerdExports) {
  const [declaration, browserLoader] = await Promise.all([
    readFile(path.join(packageDir, 'rolldown-binding.wasip1-deferred.d.ts'), 'utf8'),
    readFile(path.join(packageDir, 'rolldown-binding.wasip1-browser.js'), 'utf8'),
  ]);
  const loaderExports = new Set(findModuleExports(browserLoader));
  for (const internalExport of ['__fs', '__volume']) {
    assert.ok(loaderExports.has(internalExport), `Browser loader must retain ${internalExport}`);
  }
  const bindingExportBlock = [...declaration.matchAll(/\bexport\s*\{([^}]+)\}/gs)].find(
    ([, exports]) =>
      exports.includes('BindingBundler') && exports.includes('getRuntimeCapabilities'),
  );
  assert.ok(bindingExportBlock, 'Unable to find the Rolldown binding export list in workerd types');
  const declaredExports = new Set(
    bindingExportBlock[1]
      .split(',')
      .map((name) => name.trim())
      .filter(Boolean)
      // The dts bundler re-exports renamed locals under their public name
      // (`BindingSourceMap$1 as BindingSourceMap`); parity is about that name.
      .map((name) => name.split(/\s+as\s+/).at(-1)),
  );
  assert.deepEqual(
    runtimeExports.filter((name) => !loaderExports.has(name)).sort((a, b) => a.localeCompare(b)),
    internalRuntimeExports,
    'Only napi-rs async task implementation classes may remain raw-binding-only',
  );
  assert.deepEqual(
    [...loaderExports]
      .filter((name) => !declaredExports.has(name))
      .sort((a, b) => a.localeCompare(b)),
    // `__napiBindingTarget` IS declared since @napi-rs/cli 3.10.0 (napi-rs#3515
    // augments each flavor's preserved `.d.cts`), so it is no longer an
    // undeclared loader internal.
    ['__fs', '__volume', 'default'],
    'Only the documented browser-loader internals may be absent from binding declarations',
  );
  for (const requiredExport of [
    'getCurrentThreadTaskHostContractVersion',
    'registerCurrentThreadTaskHost',
    'registerTimerHost',
    'unregisterCurrentThreadTaskHost',
    'unregisterTimerHost',
  ]) {
    assert.ok(
      loaderExports.has(requiredExport),
      `Threadless browser loader must expose ${requiredExport}`,
    );
  }
  for (const removedExport of [
    'cancelCurrentThreadRuntimeTaskDispatch',
    'driveCurrentThreadRuntimeTasks',
  ]) {
    assert.equal(
      loaderExports.has(removedExport),
      false,
      `Threadless browser loader must not expose ${removedExport}`,
    );
  }
  const declaredWorkerdExports = collectDeclarationExports(declaration);
  assert.ok(
    declaredWorkerdExports.has('createInstance') &&
      declaredWorkerdExports.has('getWorkerdRuntimeStats'),
    'Unable to find the managed workerd declaration exports',
  );
  assert.deepEqual(
    workerdExports.filter((name) => !declaredWorkerdExports.has(name)),
    [],
    'Managed workerd declarations must cover every runtime package export',
  );
  for (const privateExport of [
    'getDeferredInstanceBinding',
    'cancelCurrentThreadRuntimeTaskDispatch',
    'driveCurrentThreadRuntimeTasks',
    'registerCurrentThreadTaskHost',
    'registerTimerHost',
    'unregisterCurrentThreadTaskHost',
    'unregisterTimerHost',
  ]) {
    assert.equal(
      declaredWorkerdExports.has(privateExport),
      false,
      `Managed workerd declarations must not expose ${privateExport}`,
    );
  }
}

async function exerciseEmbeddedWasiThreadsRefresh(consumerDir, packageDir) {
  const workerCode = await readFile(path.join(packageDir, 'wasi-worker.mjs'), 'utf8');
  const runtimeMarker = '@emnapi+wasi-threads@';
  const markerIndex = workerCode.indexOf(runtimeMarker);
  assert.notEqual(
    markerIndex,
    -1,
    'Packed threaded worker must identify its embedded wasi-threads',
  );
  const regionStart = workerCode.lastIndexOf('//#region', markerIndex);
  const regionEndMarker = '//#endregion';
  const regionEnd = workerCode.indexOf(regionEndMarker, markerIndex);
  assert.ok(regionStart >= 0 && regionEnd > regionStart, 'Unable to isolate embedded wasi-threads');
  const embeddedRuntime = workerCode.slice(0, regionEnd + regionEndMarker.length);

  const result = await runNodeModule(
    consumerDir,
    'exercise-embedded-wasi-threads-refresh.mjs',
    `${embeddedRuntime}
import assert from 'node:assert/strict'

const memory = new WebAssembly.Memory({
  initial: 1,
  maximum: 3,
  shared: true,
})
const staleBuffer = memory.buffer
memory.grow(1)

const memoryBufferGetter = Object.getOwnPropertyDescriptor(
  WebAssembly.Memory.prototype,
  'buffer',
).get
const originalGrow = memory.grow.bind(memory)
let stale = true
let refreshDelta
Object.defineProperties(memory, {
  buffer: {
    configurable: true,
    get() {
      return stale ? staleBuffer : memoryBufferGetter.call(memory)
    },
  },
  grow: {
    configurable: true,
    value(delta) {
      refreshDelta = delta
      stale = false
      return originalGrow(delta)
    },
  },
})

let spawnMessage
const wasiThreads = new WASIThreads({
  wasi: {
    initialize() {},
    start() {
      return 0
    },
  },
  childThread: true,
  postMessage(message) {
    spawnMessage = message
    const address = message.__emnapi__.payload.errorOrTid
    const struct = new Int32Array(memoryBufferGetter.call(memory), address, 2)
    Atomics.store(struct, 0, 1)
    Atomics.store(struct, 1, 6)
    Atomics.notify(struct, 1)
  },
})
wasiThreads.setup({ exports: { memory } }, {}, memory)

const errorOrTid = staleBuffer.byteLength
try {
  const spawnResult = wasiThreads.getImportObject().wasi['thread-spawn'](123, errorOrTid)
  assert.equal(spawnResult, 1)
  assert.equal(refreshDelta, 0)
  assert.equal(stale, false)
  assert.equal(spawnMessage.__emnapi__.payload.errorOrTid, errorOrTid)
  assert.deepEqual(
    Array.from(new Int32Array(memoryBufferGetter.call(memory), errorOrTid, 2)),
    [1, 6],
  )
  process.stdout.write(JSON.stringify({ refreshDelta, spawnResult }))
} finally {
  delete memory.buffer
  delete memory.grow
}
`,
    [],
  );
  assert.deepEqual(JSON.parse(result.stdout), { refreshDelta: 0, spawnResult: 1 });
  console.log('Validated packed wasi-threads refresh after stale shared-memory growth');
}

function assertThreadlessNodeLifecycle(code, loader) {
  for (const signature of [
    'function __prepareWasmEnvCleanup() {',
    'function __drainWasmEnvCleanup() {',
    'function __destroyEmnapiContext() {',
    'function __rollbackWasiInitialization() {',
    'function __runWasiInitializationRollback(record) {',
    'function __attachCleanupErrors(error, cleanupErrors) {',
    'function __registerWasiExitListener() {',
    'function __disposeWasiBinding() {',
    'function __disposeCurrentThreadHosts() {',
  ]) {
    assert.equal(
      code.split(signature).length - 1,
      1,
      `${loader} must contain exactly one ${signature}`,
    );
  }
  // The settlement barrier runs before the only raw context destroy, followed
  // by the cli 3.10.5 reentrancy guard. (Staged copies are reprinted, which
  // drops the guard's braces, so stay tolerant of formatting.)
  assert.equal(
    code.match(/__prepareWasmEnvCleanup\(\);?\n\s*if \(__isPreparingWasmEnvCleanup\(\)\)/g)?.length,
    1,
    `${loader} must run the wasm-env cleanup barrier, then the reentrancy guard, before the context destroy`,
  );
  // The settlement drain polls the wasm export and rejects retryably.
  assert.ok(
    code.includes('napi_wasm_env_cleanup_pending'),
    `${loader} must poll the wasm-env cleanup queue while draining`,
  );
  assert.match(
    code,
    /drainError\.code = ["']ERR_NAPI_WASI_CLEANUP_PENDING["'];?/,
    `${loader} must reject an undrained disposal retryably`,
  );
  assert.match(
    code,
    /Symbol\.for\(["']napi\.rs\.wasi\.dispose["']\)/,
    `${loader} must publish its disposal behind the shared dispose symbol`,
  );
  assert.match(
    code,
    /Symbol\.for\(["']napi\.rs\.wasi\.rollback\.registry\.v1["']\)/,
    `${loader} must anchor its rollback registry behind the shared symbol`,
  );
  assert.equal(
    code.match(/__wasiRollbackRegistry\.set\(__wasiRollbackRegistryKey, rollback\)/g)?.length,
    1,
    `${loader} must register its initialization rollback exactly once`,
  );
  assert.equal(
    code.match(/^\s*throw rollback\.error;?$/gm)?.length,
    1,
    `${loader} must rethrow the rollback-augmented initialization error`,
  );
  // `napi.wasm.asyncRuntime` installs the hosts from `@napi-rs/async-runtime`,
  // and the generated teardown disposes them inside `__destroyEmnapiContext`,
  // ahead of the settlement barrier and the raw destroy the rollback reaches.
  assert.match(
    code,
    /__currentThreadHostsDisposer = __installCurrentThreadHosts\(\s*__napiModule\.exports,?\s*\)/,
    `${loader} must install the CurrentThread hosts against its own binding exports`,
  );
  const hostDisposal = code.search(/__disposeCurrentThreadHosts\(\);?\n/);
  const contextDestroy = code.search(/const result = __emnapiContext\.destroy\(\);?/);
  const rollbackRegistration = code.indexOf(
    '__wasiRollbackRegistry.set(__wasiRollbackRegistryKey, rollback)',
  );
  assert.ok(
    hostDisposal !== -1 &&
      contextDestroy !== -1 &&
      rollbackRegistration !== -1 &&
      hostDisposal < contextDestroy &&
      contextDestroy < rollbackRegistration,
    `${loader} must release the CurrentThread hosts before running the initialization rollback`,
  );
}

async function exerciseThreadlessBrowserPackage(packageDir) {
  const requestedPaths = new Set();
  const server = createServer(async (request, response) => {
    response.setHeader('Cache-Control', 'no-store');
    try {
      const url = new URL(request.url ?? '/', 'http://127.0.0.1');
      requestedPaths.add(url.pathname);
      if (url.pathname === '/') {
        response.setHeader('Content-Type', 'text/html; charset=utf-8');
        response.end('<!doctype html><meta charset="utf-8"><title>Rolldown WASI</title>');
        return;
      }

      const filename = path.resolve(packageDir, `.${decodeURIComponent(url.pathname)}`);
      if (!filename.startsWith(`${packageDir}${path.sep}`)) {
        response.statusCode = 403;
        response.end('Forbidden');
        return;
      }
      response.setHeader(
        'Content-Type',
        path.extname(filename) === '.wasm' ? 'application/wasm' : 'text/javascript; charset=utf-8',
      );
      response.end(await readFile(filename));
    } catch (error) {
      response.statusCode = error?.code === 'ENOENT' ? 404 : 500;
      response.end(error instanceof Error ? error.message : String(error));
    }
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });

  const address = server.address();
  assert.ok(address && typeof address === 'object');
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    await page.goto(`http://127.0.0.1:${address.port}/`);
    const result = await withTimeout(
      page.evaluate(async () => {
        const NativeMemory = WebAssembly.Memory;
        const memories = [];
        WebAssembly.Memory = class extends NativeMemory {
          constructor(descriptor) {
            memories.push({
              initial: descriptor.initial,
              maximum: descriptor.maximum,
              shared: descriptor.shared === true,
            });
            super(descriptor);
          }
        };

        try {
          const bindingSpecifier = ['./rolldown-binding', 'wasip1-browser.js'].join('.');
          const binding = await import(bindingSpecifier);
          const bundler = new binding.BindingBundler();
          let outputs;
          try {
            const buildResult = await bundler.generate({
              inputOptions: {
                input: [{ import: 'virtual:entry' }],
                plugins: [
                  {
                    name: 'threadless-browser-packed-consumer',
                    hookUsage: 11,
                    resolveId(_ctx, id) {
                      if (id === 'virtual:entry') return { id };
                    },
                    load(_ctx, id) {
                      if (id === 'virtual:entry') return { code: 'export default 1' };
                    },
                  },
                ],
                cwd: '/',
                logLevel: 0,
                onLog() {},
              },
              outputOptions: { format: 'es', plugins: [] },
            });
            if ('isBindingErrors' in buildResult) {
              throw new Error(JSON.stringify(buildResult.errors));
            }
            outputs = buildResult.chunks.length + buildResult.assets.length;
          } finally {
            await bundler.close();
          }
          return {
            outputs,
            memories,
            capabilities: binding.getRuntimeCapabilities(),
          };
        } finally {
          WebAssembly.Memory = NativeMemory;
        }
      }),
      browserExerciseTimeoutMs,
      'Threadless WASI Chromium exercise',
    );
    assert.deepEqual(pageErrors, []);
    assert.ok(
      requestedPaths.has('/rolldown-binding.wasm32-wasip1.wasm'),
      'Threadless WASI browser loader did not fetch the packed Wasm artifact',
    );
    return result;
  } finally {
    try {
      await browser?.close();
    } finally {
      server.closeAllConnections?.();
      await new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
    }
  }
}

async function exerciseBrowserPackageRoot(packageDir) {
  const requestedPaths = new Set();
  const server = createServer(async (request, response) => {
    response.setHeader('Cache-Control', 'no-store');
    try {
      const url = new URL(request.url ?? '/', 'http://127.0.0.1');
      requestedPaths.add(url.pathname);
      if (url.pathname === '/') {
        response.setHeader('Content-Type', 'text/html; charset=utf-8');
        response.end('<!doctype html><meta charset="utf-8"><title>Rolldown browser API</title>');
        return;
      }

      const filename = path.resolve(packageDir, `.${decodeURIComponent(url.pathname)}`);
      if (!filename.startsWith(`${packageDir}${path.sep}`)) {
        response.statusCode = 403;
        response.end('Forbidden');
        return;
      }
      const extension = path.extname(filename);
      response.setHeader(
        'Content-Type',
        extension === '.wasm'
          ? 'application/wasm'
          : extension === '.js' || extension === '.mjs'
            ? 'text/javascript; charset=utf-8'
            : 'application/octet-stream',
      );
      response.end(await readFile(filename));
    } catch (error) {
      response.statusCode = error?.code === 'ENOENT' ? 404 : 500;
      response.end(error instanceof Error ? error.message : String(error));
    }
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });

  const address = server.address();
  assert.ok(address && typeof address === 'object');
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    await page.goto(`http://127.0.0.1:${address.port}/`);
    const result = await withTimeout(
      page.evaluate(async () => {
        Object.defineProperty(globalThis, 'AsyncContext', {
          configurable: true,
          value: undefined,
          writable: true,
        });

        const [browserApi, experimentalApi] = await Promise.all([
          import('./dist/index.browser.mjs'),
          import('./dist/experimental-index.browser.mjs'),
        ]);

        experimentalApi.memfs.volume.fromJSON({
          '/entry.js': 'export default 1',
        });
        const callbackFreeBundle = await browserApi.rolldown({
          cwd: '/',
          input: '/entry.js',
        });
        try {
          await callbackFreeBundle.generate();
        } finally {
          await callbackFreeBundle.close();
        }

        const callbackCalls = {
          buildStart: 0,
          load: 0,
          resolveId: 0,
        };
        let reentrantBuild = Promise.resolve();
        let callbackBundle;
        const callbackPlugin = {
          name: 'packed-browser-root',
          async buildStart() {
            callbackCalls.buildStart += 1;
            await Promise.resolve();
            reentrantBuild = callbackBundle.generate().catch((error) => error);
            await reentrantBuild;
          },
          resolveId(id) {
            callbackCalls.resolveId += 1;
            if (id === 'virtual:entry') return id;
          },
          load(id) {
            callbackCalls.load += 1;
            if (id === 'virtual:entry') return 'export default 1';
          },
        };
        if (
          typeof Object.getOwnPropertyDescriptor(callbackPlugin, 'buildStart')?.value !== 'function'
        ) {
          throw new Error('Browser preflight exercise requires a direct callback data property');
        }
        callbackBundle = await browserApi.rolldown({
          cwd: '/',
          input: 'virtual:entry',
          plugins: [callbackPlugin],
        });

        let unavailableError;
        try {
          await callbackBundle.generate();
        } catch (error) {
          unavailableError = {
            code: error?.code,
            message: error instanceof Error ? error.message : String(error),
            name: error?.name,
          };
        }
        if (!unavailableError) {
          throw new Error('Browser callback unexpectedly ran without an async-context provider');
        }
        if (callbackCalls.buildStart !== 0) {
          throw new Error('Unavailable async context did not fail before the browser callback');
        }

        const initialSupport = experimentalApi.getAsyncContextSupport();
        let storageCreations = 0;
        let storageRuns = 0;
        const provider = {
          createStorage() {
            storageCreations += 1;
            let current;
            return {
              getStore() {
                return current;
              },
              run(store, callback) {
                storageRuns += 1;
                const previous = current;
                current = store;
                let result;
                try {
                  result = callback();
                } catch (error) {
                  current = previous;
                  throw error;
                }
                let then;
                try {
                  then =
                    result !== null && (typeof result === 'object' || typeof result === 'function')
                      ? result.then
                      : undefined;
                } catch (error) {
                  current = previous;
                  throw error;
                }
                if (typeof then === 'function') {
                  return Promise.resolve(result).finally(() => {
                    current = previous;
                  });
                }
                current = previous;
                return result;
              },
            };
          },
        };
        experimentalApi.configureAsyncContext(provider);

        const providerProbe = provider.createStorage();
        const providerStoreAfterAwait = await providerProbe.run('propagated', async () => {
          await Promise.resolve();
          return providerProbe.getStore();
        });
        const transformResult = await experimentalApi.transform(
          'packed-browser.ts',
          'const answer: number = 42;',
        );

        const metadataCause = Object.assign(new RangeError('browser nested cause'), {
          nestedMarker: 37,
        });
        const originalMetadataError = Object.assign(
          new TypeError('browser plugin metadata failure'),
          {
            cause: metadataCause,
            code: 'BROWSER_USER_CODE',
            customMarker: 'browser-retained',
          },
        );
        const metadataBundle = await browserApi.rolldown({
          cwd: '/',
          input: 'virtual:metadata',
          plugins: [
            {
              name: 'packed-browser-metadata',
              resolveId(id) {
                if (id === 'virtual:metadata') return id;
              },
              load(id) {
                if (id === 'virtual:metadata') return 'export default 1';
              },
              transform(_code, id) {
                if (id === 'virtual:metadata') throw originalMetadataError;
              },
            },
          ],
        });
        let pluginError;
        try {
          const failure = await metadataBundle.generate().catch((error) => error);
          const [error] = failure?.errors ?? [];
          pluginError = {
            causeIdentity: error?.cause === metadataCause,
            code: error?.code,
            customMarker: error?.customMarker,
            hook: error?.hook,
            id: error?.id,
            identity: error === originalMetadataError,
            nestedMarker: error?.cause?.nestedMarker,
            plugin: error?.plugin,
            pluginCode: error?.pluginCode,
            stackRetained: /browser plugin metadata failure/.test(error?.stack ?? ''),
          };
        } finally {
          await metadataBundle.close();
        }

        let outputs;
        try {
          const buildResult = await callbackBundle.generate();
          outputs = buildResult.output.length;
        } finally {
          await callbackBundle.close();
        }
        const reentrantError = await reentrantBuild;

        return {
          callbackCalls,
          initialSupport,
          outputs,
          pluginError,
          providerStoreAfterAwait,
          reentrantError: reentrantError?.message,
          storageCreations,
          storageRuns,
          support: experimentalApi.getAsyncContextSupport(),
          transform: {
            code: transformResult.code,
            errors: transformResult.errors.length,
            warnings: transformResult.warnings.length,
          },
          unavailableError,
        };
      }),
      browserExerciseTimeoutMs,
      'Packed @rolldown/browser Chromium exercise',
    );
    assert.deepEqual(pageErrors, []);
    assert.ok(
      requestedPaths.has('/dist/rolldown-binding.wasm32-wasip1.wasm'),
      'Packed @rolldown/browser did not fetch its Wasm artifact in Chromium',
    );
    return result;
  } finally {
    try {
      await browser?.close();
    } finally {
      server.closeAllConnections?.();
      await new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
    }
  }
}

async function exerciseThreadedBrowserPackage(packageDir) {
  const requestedPaths = new Set();
  const server = createServer(async (request, response) => {
    response.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
    response.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    response.setHeader('Cross-Origin-Resource-Policy', 'same-origin');
    response.setHeader('Cache-Control', 'no-store');

    try {
      const url = new URL(request.url ?? '/', 'http://127.0.0.1');
      requestedPaths.add(url.pathname);
      if (url.pathname === '/') {
        response.setHeader('Content-Type', 'text/html; charset=utf-8');
        response.end('<!doctype html><meta charset="utf-8"><title>Rolldown WASI</title>');
        return;
      }

      const filename = path.resolve(packageDir, `.${decodeURIComponent(url.pathname)}`);
      if (!filename.startsWith(`${packageDir}${path.sep}`)) {
        response.statusCode = 403;
        response.end('Forbidden');
        return;
      }
      const extension = path.extname(filename);
      response.setHeader(
        'Content-Type',
        extension === '.wasm'
          ? 'application/wasm'
          : extension === '.js' || extension === '.mjs'
            ? 'text/javascript; charset=utf-8'
            : 'application/octet-stream',
      );
      response.end(await readFile(filename));
    } catch (error) {
      response.statusCode = error?.code === 'ENOENT' ? 404 : 500;
      response.end(error instanceof Error ? error.message : String(error));
    }
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });

  const address = server.address();
  assert.ok(address && typeof address === 'object');
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    await page.goto(`http://127.0.0.1:${address.port}/`);
    const result = await withTimeout(
      page.evaluate(async () => {
        if (!globalThis.crossOriginIsolated) {
          throw new Error('Threaded WASI browser test is not cross-origin isolated');
        }
        if (typeof globalThis.SharedArrayBuffer !== 'function') {
          throw new Error('SharedArrayBuffer is unavailable in the isolated browser page');
        }

        const NativeWorker = globalThis.Worker;
        const NativeMemory = WebAssembly.Memory;
        let workers = 0;
        let sharedMemories = 0;
        let workerEventErrors = 0;
        let workerMessageErrors = 0;
        globalThis.addEventListener('napi-rs-worker-error', () => {
          workerMessageErrors += 1;
        });
        globalThis.Worker = class extends NativeWorker {
          constructor(url, options) {
            workers += 1;
            super(url, options);
            this.addEventListener('error', () => {
              workerEventErrors += 1;
            });
          }
        };
        WebAssembly.Memory = class extends NativeMemory {
          constructor(descriptor) {
            if (descriptor.shared) sharedMemories += 1;
            super(descriptor);
          }
        };

        try {
          const bindingSpecifier = ['./rolldown-binding', 'wasi-browser.js'].join('.');
          const binding = await import(bindingSpecifier);
          const bundler = new binding.BindingBundler();
          let outputs;
          try {
            const buildResult = await bundler.generate({
              inputOptions: {
                input: [{ import: 'virtual:entry' }],
                plugins: [
                  {
                    name: 'threaded-browser-packed-consumer',
                    hookUsage: 11,
                    resolveId(_ctx, id) {
                      if (id === 'virtual:entry') return { id };
                    },
                    load(_ctx, id) {
                      if (id === 'virtual:entry') return { code: 'export default 1' };
                    },
                  },
                ],
                cwd: '/',
                logLevel: 0,
                onLog() {},
              },
              outputOptions: { format: 'es', plugins: [] },
            });
            if ('isBindingErrors' in buildResult) {
              throw new Error(JSON.stringify(buildResult.errors));
            }
            outputs = buildResult.chunks.length + buildResult.assets.length;
          } finally {
            await bundler.close();
          }
          await new Promise((resolve) => setTimeout(resolve, 25));
          return {
            crossOriginIsolated: globalThis.crossOriginIsolated,
            sharedArrayBuffer: typeof globalThis.SharedArrayBuffer,
            sharedMemories,
            workers,
            workerEventErrors,
            workerMessageErrors,
            outputs,
            capabilities: binding.getRuntimeCapabilities(),
          };
        } finally {
          globalThis.Worker = NativeWorker;
          WebAssembly.Memory = NativeMemory;
        }
      }),
      browserExerciseTimeoutMs,
      'Threaded WASI Chromium exercise',
    );
    assert.deepEqual(pageErrors, []);
    assert.ok(
      requestedPaths.has('/rolldown-binding.wasm32-wasi.wasm'),
      'Threaded WASI browser loader did not fetch the packed Wasm artifact',
    );
    assert.ok(
      requestedPaths.has('/wasi-worker-browser.mjs'),
      'Threaded WASI browser loader did not fetch the packed worker artifact',
    );
    return result;
  } finally {
    try {
      await browser?.close();
    } finally {
      server.closeAllConnections?.();
      await new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
    }
  }
}

const exerciseSource = `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'

const [workerdSpecifier, wasmSpecifier] = process.argv.slice(2)
const workerd = await import(workerdSpecifier)
const require = createRequire(import.meta.url)
const wasmModule = await WebAssembly.compile(await readFile(require.resolve(wasmSpecifier)))
if (typeof workerd.createInstance !== 'function') throw new Error('Missing workerd createInstance')
const instance = await workerd.createInstance(wasmModule)
let grew = false
const bundler = new instance.exports.BindingBundler()
const retainedCapabilities = instance.exports.getRuntimeCapabilities
const RetainedBundler = instance.exports.BindingBundler
assert.throws(
  () => Reflect.set(bundler, 'close', async () => {}),
  /Cannot replace or remove close/,
)
assert.throws(
  () => Reflect.set(Object.getPrototypeOf(bundler), 'close', async () => {}),
  /Cannot replace or remove close/,
)
try {
  const result = await bundler.generate({
    inputOptions: {
      input: [{ import: 'virtual:entry' }],
      plugins: [{
        name: 'memory-growth',
        hookUsage: 8203,
        buildStart() {
          instance.memory.grow(1)
          grew = true
        },
        resolveId(_ctx, id) {
          if (id === 'virtual:entry') return { id }
        },
        load(_ctx, id) {
          if (id === 'virtual:entry') return { code: 'export default 1' }
        },
        closeBundle() {
          throw Object.assign(new Error('managed raw close failure'), {
            code: 'ERR_MANAGED_RAW_CLOSE',
          })
        },
      }],
      cwd: '/',
      logLevel: 0,
      onLog() {},
    },
    outputOptions: { format: 'es', plugins: [] },
  })
  if ('isBindingErrors' in result) throw new Error(JSON.stringify(result.errors))
  process.stdout.write(JSON.stringify({
    grew,
    outputs: result.chunks.length + result.assets.length,
    capabilities: instance.exports.getRuntimeCapabilities(),
    runtimeExports: Object.keys(instance.exports).sort((a, b) => a.localeCompare(b)),
    workerdExports: Object.keys(workerd).sort((a, b) => a.localeCompare(b)),
    resolution: {
      workerd: import.meta.resolve(workerdSpecifier),
      wasm: import.meta.resolve(wasmSpecifier),
    },
  }))
} finally {
  try {
    const closeError = await bundler.close().catch((error) => error)
    assert.equal(closeError?.message, 'managed raw close failure')
    assert.equal(closeError?.code, 'ERR_MANAGED_RAW_CLOSE')
  } finally {
    await instance.dispose()
  }
}
let retainedFailures = 0
try {
  retainedCapabilities()
} catch (error) {
  if (/This workerd Rolldown instance has been disposed/.test(error.message)) {
    retainedFailures += 1
  } else {
    throw error
  }
}
try {
  new RetainedBundler()
} catch (error) {
  if (/This workerd Rolldown instance has been disposed/.test(error.message)) {
    retainedFailures += 1
  } else {
    throw error
  }
}
if (retainedFailures !== 2) {
  throw new Error('Retained managed binding aliases remained callable after disposal')
}
`;

const directBindingExerciseSource = `
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'

const [bindingSpecifier] = process.argv.slice(2)
const require = createRequire(import.meta.url)
const binding = require(bindingSpecifier)
for (const removedExport of [
  'cancelCurrentThreadRuntimeTaskDispatch',
  'driveCurrentThreadRuntimeTasks',
]) {
  assert.equal(removedExport in binding, false)
}
assert.equal(binding.getCurrentThreadTaskHostContractVersion(), 4)
// Contract v4 reserved-token protocol, driven against the packed artifact.
const assertRegistrationActive = (registration, expected) =>
  assert.equal(
    binding.isCurrentThreadHostRegistrationActive(registration.high, registration.low),
    expected,
  )
const assertReservationShape = (registration) => {
  assert.equal(Number.isInteger(registration.high), true)
  assert.equal(registration.high >= 0 && registration.high <= 0xffffffff, true)
  assert.equal(Number.isInteger(registration.low), true)
  assert.equal(registration.low >= 0 && registration.low <= 0xffffffff, true)
  assert.equal(registration.high === 0 && registration.low === 0, false)
}
const taskReservation = binding.reserveCurrentThreadHostRegistration()
assertReservationShape(taskReservation)
assertRegistrationActive(taskReservation, false)
// A rejected dispatch callback fails before the reservation is consumed...
assert.throws(
  () =>
    binding.registerCurrentThreadTaskHost(taskReservation.high, taskReservation.low, () => {}),
  /registerCurrentThreadTaskHost does not accept a JavaScript callback/,
)
assertRegistrationActive(taskReservation, false)
// ...so the same token still registers, reports live, and unregisters.
binding.registerCurrentThreadTaskHost(taskReservation.high, taskReservation.low)
assertRegistrationActive(taskReservation, true)
// A consumed token cannot be registered a second time.
assert.throws(
  () => binding.registerCurrentThreadTaskHost(taskReservation.high, taskReservation.low),
  /was not reserved or was already consumed/,
)
assertRegistrationActive(taskReservation, true)
binding.unregisterCurrentThreadTaskHost(taskReservation.high, taskReservation.low)
assertRegistrationActive(taskReservation, false)
const timerReservation = binding.reserveCurrentThreadHostRegistration()
assertReservationShape(timerReservation)
assert.equal(
  timerReservation.high === taskReservation.high && timerReservation.low === taskReservation.low,
  false,
)
assertRegistrationActive(timerReservation, false)
// The task token released above stays consumed for timer registration too.
assert.throws(
  () =>
    binding.registerTimerHost(
      taskReservation.high,
      taskReservation.low,
      () => Promise.resolve(),
      () => {},
    ),
  /was not reserved or was already consumed/,
)
binding.registerTimerHost(
  timerReservation.high,
  timerReservation.low,
  () => Promise.resolve(),
  () => {},
)
assertRegistrationActive(timerReservation, true)
binding.unregisterTimerHost(timerReservation.high, timerReservation.low)
assertRegistrationActive(timerReservation, false)
const bundler = new binding.BindingBundler()
try {
  const result = await bundler.generate({
    inputOptions: {
      input: [{ import: 'virtual:entry' }],
      plugins: [{
        name: 'direct-threadless-binding-consumer',
        hookUsage: 8203,
        resolveId(_ctx, id) {
          if (id === 'virtual:entry') return { id }
        },
        load(_ctx, id) {
          if (id === 'virtual:entry') return { code: 'export default 1' }
        },
        closeBundle() {
          throw Object.assign(new Error('direct raw close failure'), {
            code: 'ERR_DIRECT_RAW_CLOSE',
          })
        },
      }],
      cwd: '/',
      logLevel: 0,
      onLog() {},
    },
    outputOptions: { format: 'es', plugins: [] },
  })
  if ('isBindingErrors' in result) throw new Error(JSON.stringify(result.errors))
  process.stdout.write(JSON.stringify({
    outputs: result.chunks.length + result.assets.length,
    capabilities: binding.getRuntimeCapabilities(),
    taskHostContractVersion: binding.getCurrentThreadTaskHostContractVersion(),
  }))
} finally {
  const closeError = await bundler.close().catch((error) => error)
  assert.equal(closeError?.message, 'direct raw close failure')
  assert.equal(closeError?.code, 'ERR_DIRECT_RAW_CLOSE')
}
`;

const browserBindingExerciseSource = `
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const [bindingSpecifier] = process.argv.slice(2)
const nativeFetch = globalThis.fetch
globalThis.fetch = async (input, init) => {
  const url = new URL(typeof input === 'string' || input instanceof URL ? input : input.url)
  if (url.protocol !== 'file:') return nativeFetch(input, init)
  const bytes = await readFile(url)
  return {
    ok: true,
    status: 200,
    statusText: 'OK',
    arrayBuffer: async () =>
      bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
  }
}

try {
  const binding = await import(bindingSpecifier)
  for (const removedExport of [
    'cancelCurrentThreadRuntimeTaskDispatch',
    'driveCurrentThreadRuntimeTasks',
  ]) {
    assert.equal(removedExport in binding, false)
    assert.equal(removedExport in binding.default, false)
  }
  assert.equal(binding.getCurrentThreadTaskHostContractVersion(), 4)
  // Contract v4 reserved-token protocol against the browser-condition artifact.
  const taskReservation = binding.reserveCurrentThreadHostRegistration()
  assert.equal(Number.isInteger(taskReservation.high), true)
  assert.equal(taskReservation.high >= 0 && taskReservation.high <= 0xffffffff, true)
  assert.equal(Number.isInteger(taskReservation.low), true)
  assert.equal(taskReservation.low >= 0 && taskReservation.low <= 0xffffffff, true)
  assert.equal(taskReservation.high === 0 && taskReservation.low === 0, false)
  assert.equal(
    binding.isCurrentThreadHostRegistrationActive(taskReservation.high, taskReservation.low),
    false,
  )
  // A rejected dispatch callback fails before the reservation is consumed.
  assert.throws(
    () =>
      binding.registerCurrentThreadTaskHost(taskReservation.high, taskReservation.low, () => {}),
    /registerCurrentThreadTaskHost does not accept a JavaScript callback/,
  )
  assert.equal(
    binding.isCurrentThreadHostRegistrationActive(taskReservation.high, taskReservation.low),
    false,
  )
  binding.registerCurrentThreadTaskHost(taskReservation.high, taskReservation.low)
  assert.equal(
    binding.isCurrentThreadHostRegistrationActive(taskReservation.high, taskReservation.low),
    true,
  )
  binding.unregisterCurrentThreadTaskHost(taskReservation.high, taskReservation.low)
  assert.equal(
    binding.isCurrentThreadHostRegistrationActive(taskReservation.high, taskReservation.low),
    false,
  )
  assert.throws(
    () => binding.registerCurrentThreadTaskHost(taskReservation.high, taskReservation.low),
    /was not reserved or was already consumed/,
  )
  const bundler = new binding.BindingBundler()
  try {
    const result = await bundler.generate({
      inputOptions: {
        input: [{ import: 'virtual:entry' }],
        plugins: [{
          name: 'browser-condition-threadless-binding-consumer',
          hookUsage: 11,
          resolveId(_ctx, id) {
            if (id === 'virtual:entry') return { id }
          },
          load(_ctx, id) {
            if (id === 'virtual:entry') return { code: 'export default 1' }
          },
        }],
        cwd: '/',
        logLevel: 0,
        onLog() {},
      },
      outputOptions: { format: 'es', plugins: [] },
    })
    if ('isBindingErrors' in result) throw new Error(JSON.stringify(result.errors))
    process.stdout.write(JSON.stringify({
      outputs: result.chunks.length + result.assets.length,
      capabilities: binding.getRuntimeCapabilities(),
      taskHostContractVersion: binding.getCurrentThreadTaskHostContractVersion(),
      resolved: import.meta.resolve(bindingSpecifier),
    }))
  } finally {
    await bundler.close()
  }
} finally {
  globalThis.fetch = nativeFetch
}
`;

const memoryFloorExerciseSource = `
import { readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'

const [workerdSpecifier, wasmSpecifier] = process.argv.slice(2)
const workerd = await import(workerdSpecifier)
const require = createRequire(import.meta.url)
const wasmModule = await WebAssembly.compile(await readFile(require.resolve(wasmSpecifier)))
const moduleCount = 256
const rounds = 3
const samples = []

for (let round = 1; round <= rounds; round += 1) {
  const instance = await workerd.createInstance(wasmModule)
  const initialMemoryBytes = instance.memoryBytes
  const bundler = new instance.exports.BindingBundler()
  try {
    const result = await bundler.generate({
      inputOptions: {
        input: [{ import: 'virtual:0' }],
        plugins: [{
          name: 'workerd-memory-floor',
          hookUsage: 11,
          resolveId(_ctx, id) {
            if (id.startsWith('virtual:')) return { id }
          },
          load(_ctx, id) {
            if (!id.startsWith('virtual:')) return
            const index = Number(id.slice('virtual:'.length))
            if (index + 1 < moduleCount) {
              return {
                code:
                  "import value from 'virtual:" +
                  (index + 1) +
                  "'; export default value + " +
                  index,
              }
            }
            return { code: 'export default 1' }
          },
        }],
        cwd: '/',
        logLevel: 0,
        onLog() {},
      },
      outputOptions: { format: 'es', plugins: [] },
    })
    if ('isBindingErrors' in result) throw new Error(JSON.stringify(result.errors))
    samples.push({
      round,
      initialMemoryBytes,
      afterBuildMemoryBytes: instance.memoryBytes,
      outputs: result.chunks.length + result.assets.length,
    })
  } finally {
    try {
      await bundler.close()
    } finally {
      await instance.dispose()
    }
  }
}

process.stdout.write(JSON.stringify({
  moduleCount,
  rounds,
  memory: workerd.WORKERD_WASM_MEMORY,
  samples,
  stats: workerd.getWorkerdRuntimeStats(),
}))
`;

const browserPackageExerciseSource = `
import { AsyncLocalStorage } from 'node:async_hooks'
import { readFile } from 'node:fs/promises'

const nativeFetch = globalThis.fetch
globalThis.fetch = async (input, init) => {
  const url = new URL(typeof input === 'string' || input instanceof URL ? input : input.url)
  if (url.protocol !== 'file:') return nativeFetch(input, init)
  const bytes = await readFile(url)
  return {
    ok: true,
    status: 200,
    statusText: 'OK',
    arrayBuffer: async () =>
      bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
  }
}

try {
  const [
    { rolldown },
    {
      configureAsyncContext,
      getRuntimeCapabilities,
      getRuntimeSupport,
    },
  ] = await Promise.all([
      import('@rolldown/browser'),
      import('@rolldown/browser/experimental'),
    ])
  configureAsyncContext({
    createStorage: () => new AsyncLocalStorage(),
  })
  const bundle = await rolldown({
    input: 'virtual:entry',
    plugins: [{
      name: 'browser-package-root-consumer',
      resolveId(id) {
        if (id === 'virtual:entry') return id
      },
      load(id) {
        if (id === 'virtual:entry') return 'export default 1'
      },
    }],
  })
  try {
    const result = await bundle.generate()
    const support = getRuntimeSupport()
    process.stdout.write(JSON.stringify({
      outputs: result.output.length,
      capabilities: getRuntimeCapabilities(),
      support,
      resolution: {
        root: import.meta.resolve('@rolldown/browser'),
        experimental: import.meta.resolve('@rolldown/browser/experimental'),
      },
    }))
  } finally {
    await bundle.close()
  }
} finally {
  globalThis.fetch = nativeFetch
}
`;

const rootPackageExerciseSource = `
import { rolldown } from 'rolldown'
import {
  getRuntimeCapabilities,
  getRuntimeSupport,
} from 'rolldown/experimental'

const bundle = await rolldown({
  input: 'virtual:entry',
  plugins: [{
    name: 'root-package-public-api-consumer',
    resolveId(id) {
      if (id === 'virtual:entry') return id
    },
    load(id) {
      if (id === 'virtual:entry') return 'export default 1'
    },
  }],
})
try {
  const result = await bundle.generate()
  process.stdout.write(JSON.stringify({
    outputs: result.output.length,
    capabilities: getRuntimeCapabilities(),
    support: getRuntimeSupport(),
  }))
} finally {
  await bundle.close()
}
`;

// Disposing the threaded flavor only means anything once a build has actually
// spawned the WASI pool workers, so this exercise bundles first and disposes
// the very module instance that build ran on.
const threadedDisposeExerciseSource = `
import { createRequire } from 'node:module'

const disposeTimeoutMs = 30_000
// A stray worker exit lands on the tick right after the disposal settles, so
// hold the process open long enough for one to surface. This timer stays
// REFERENCED on purpose: it only ever runs once the disposal has settled, so it
// cannot stand in for the liveness the loader owes while one is in flight.
const drainMs = 500
// How long a pool worker may take to emit 'exit' after that.
const workerExitWaitMs = 5_000

const nodeRequire = createRequire(import.meta.url)
const { writeSync } = nodeRequire('node:fs')
const workerThreads = nodeRequire('node:worker_threads')
const NativeWorker = workerThreads.Worker
const workers = []
const workerErrors = []
// Patch before the binding loads: the generated loader destructures Worker the
// first time it is required. Every constructed worker is retained with its own
// 'exit' record: the pool workers are unreferenced, so without this the process
// could exit over a still-running worker and call the teardown a success.
workerThreads.Worker = class extends NativeWorker {
  constructor(filename, options) {
    super(filename, options)
    const record = { exited: false }
    workers.push(record)
    this.on('error', (error) => {
      workerErrors.push(error && error.message ? error.message : String(error))
    })
    this.on('exit', () => {
      record.exited = true
    })
  }
}
const countExitedWorkers = () => workers.filter((worker) => worker.exited).length

const { rolldown } = await import('rolldown')
const { getRuntimeCapabilities } = await import('rolldown/experimental')

const bundle = await rolldown({
  input: 'virtual:entry',
  plugins: [{
    name: 'root-package-threaded-dispose-consumer',
    resolveId(id) {
      if (id === 'virtual:entry') return id
    },
    load(id) {
      if (id === 'virtual:entry') return 'export default 1'
    },
  }],
})
let outputs
try {
  outputs = (await bundle.generate()).output.length
} finally {
  await bundle.close()
}
// Read every binding-backed value up front. Once the binding is disposed its
// emnapi context is gone, so any later call into it traps the wasm module.
const target = getRuntimeCapabilities().target

// Read the binding out of the require cache rather than loading a second copy:
// only the instance the build ran on owns the workers that must go away.
const rootRequire = createRequire(nodeRequire.resolve('rolldown/package.json'))
const bindingEntry = rootRequire.resolve('@rolldown/binding-wasm32-wasi')
const loadedBinding = rootRequire.cache[bindingEntry]
if (!loadedBinding) {
  throw new Error('The build did not load ' + bindingEntry)
}
const dispose = loadedBinding.exports[Symbol.for('napi.rs.wasi.dispose')]
if (typeof dispose !== 'function') {
  throw new Error(
    'The threaded WASI binding must publish a dispose function, received ' + typeof dispose,
  )
}

// A crash during disposal has to fail this consumer instead of being swallowed.
// Write synchronously: the immediate exit would drop a buffered pipe write.
const failDisposal = (reason) => (error) => {
  writeSync(
    2,
    'THREADED WASI DISPOSE FAILED (' + reason + '): ' +
      (error && error.stack ? error.stack : String(error)) + '\\n',
  )
  process.exit(1)
}
process.on('uncaughtException', failDisposal('uncaughtException'))
process.on('unhandledRejection', failDisposal('unhandledRejection'))

// The driver's withTimeout lives in the parent process, so mirror it here --
// but UNREFERENCED. Holding the event loop open across a pending disposal is
// the loader's own job; a referenced watchdog here would supply that liveness
// and hide its loss. Without it Node simply exits mid-disposal, and the driver
// reads the missing marker as exactly that.
let disposeTimeout
let disposed = false
try {
  await Promise.race([
    Promise.resolve(dispose()).then(() => {
      disposed = true
    }),
    new Promise((_, reject) => {
      disposeTimeout = setTimeout(
        () =>
          reject(
            new Error('The threaded WASI disposal never settled after ' + disposeTimeoutMs + 'ms'),
          ),
        disposeTimeoutMs,
      )
      disposeTimeout.unref()
    }),
  ])
} finally {
  clearTimeout(disposeTimeout)
}
await new Promise((resolve) => setTimeout(resolve, drainMs))

// A settled disposal is not a terminated pool. Wait, bounded, until every
// worker the loader built has reported 'exit'. This poll is REFERENCED so the
// mismatch is always reported rather than turning into a silent exit: it runs
// only once the disposal has settled, which is strictly after the unreferenced
// watchdog above has done its job, so it cannot stand in for the loader's own
// keep-alive either.
const workerExitDeadline = Date.now() + workerExitWaitMs
while (countExitedWorkers() < workers.length && Date.now() < workerExitDeadline) {
  await new Promise((resolve) => setTimeout(resolve, 50))
}

process.stdout.write(JSON.stringify({
  marker: 'threaded-wasi-dispose',
  target,
  outputs,
  disposed,
  workers: workers.length,
  exitedWorkers: countExitedWorkers(),
  workerErrors,
}))
`;

// Disposal has to survive being called while the addon still owes completion
// callbacks, not just between operations.
//
// `napi_prepare_wasm_env_cleanup` — the barrier the settlement drain already
// used — covers promise settlements sitting on the threadsafe-function queue.
// It does not cover `napi_async_work`. A loader that destroyed the emnapi
// context (or terminated the WASI pool workers) with a work outstanding left it
// with no way to reach its completion callback: the promise never settled, and
// the emnapi waiting-request counter that brackets every queued work never
// returned to zero, which on Node keeps a `MessageChannel` port referenced and
// the process alive forever. napi-rs#3528 closes that: the napi crate exports
// `napi_wasm_async_work_pending` / `napi_wasm_cancel_pending_async_work`, and
// @napi-rs/cli 3.10.2 drains through them (`__drainWasiAsyncWork` in the eager
// loaders) before anything is torn down.
//
// Two things have to be in flight at once for this to mean anything:
//
//   - A real bundle build, because that is the shape a consumer disposes over.
//     It does NOT by itself put anything in the async-work registry: rolldown
//     runs bundling on the pluggable async runtime
//     (`spawn_boxed_future`, crates/rolldown_binding/src/binding_bundler.rs),
//     not on `napi_async_work`. Measured against this very build: the counter
//     reads 0 for a bundle build's whole duration.
//   - `transform()`, which is rolldown's one `napi::Task`
//     (`EnhancedTransformTask`, crates/rolldown_binding/src/transform.rs), so
//     it is what actually queues `napi_async_work`.
//
// Only the two eager loaders are covered here. The deferred loader's own drain
// (`__drainInstanceAsyncWork`) is unreachable from a packed consumer: the
// workerd path goes through `packages/rolldown/src/workerd-managed-instance.ts`,
// whose facade refuses to dispose an instance with active binding operations at
// all, and `scripts/wasi/check-workerd-packed-consumer.mjs` only ever reaches
// that facade. napi-rs owns that half's coverage.
//
// Outstanding-ness is structural rather than timed: `transform()` queues its
// work synchronously, completion callbacks are delivered on the JavaScript
// thread, and nothing yields between the burst and `dispose()`. So every one of
// them is still owed a callback at the moment disposal starts, on any machine.
// Then each one has to settle — fulfilled if it ran to completion, or rejected
// with the `AbortError` napi-rs turns `napi_cancelled` into. A loader without
// the drain settles none of them and the child never exits, which is what
// `runDisposeInFlightExercise` reports.
function renderDisposeInFlightExerciseSource({ bindingPackage, marker }) {
  return `
import { createRequire } from 'node:module'

const disposeTimeoutMs = 30_000
// How long a settlement may take AFTER the disposal has settled. Reached only
// when work was stranded, so it is a diagnosis budget, not a wait anyone pays
// in the healthy case.
const settleWaitMs = 15_000
// How long a pool worker may take to emit 'exit' after that.
const workerExitWaitMs = 5_000
// Enough that the pool cannot have started, let alone finished, all of them.
const asyncWorkCount = 8

const nodeRequire = createRequire(import.meta.url)
const { writeSync } = nodeRequire('node:fs')
const workerThreads = nodeRequire('node:worker_threads')
const NativeWorker = workerThreads.Worker
const workers = []
const workerErrors = []
// Patch before the binding loads, exactly as the dispose-after-build exercise
// does: the generated loader destructures Worker on first require, and the pool
// workers are unreferenced, so nothing else would keep them observable.
workerThreads.Worker = class extends NativeWorker {
  constructor(filename, options) {
    super(filename, options)
    const record = { exited: false }
    workers.push(record)
    this.on('error', (error) => {
      workerErrors.push(error && error.message ? error.message : String(error))
    })
    this.on('exit', () => {
      record.exited = true
    })
  }
}
const countExitedWorkers = () => workers.filter((worker) => worker.exited).length

const { rolldown } = await import('rolldown')
const { getRuntimeCapabilities, transform } = await import('rolldown/experimental')

// The build has to be inside the binding when disposal starts, not merely
// requested from JavaScript. A load hook only runs once the Rust side is
// driving the build, so it is the signal; holding it open keeps the build
// unsettled across the disposal without the exercise having to guess a delay.
let signalBuildInBinding
const buildReachedBinding = new Promise((resolve) => {
  signalBuildInBinding = resolve
})
let releaseBuild
const buildHeld = new Promise((resolve) => {
  releaseBuild = resolve
})

const bundle = await rolldown({
  input: 'virtual:entry',
  plugins: [{
    name: 'root-package-dispose-in-flight-consumer',
    resolveId(id) {
      if (id === 'virtual:entry') return id
    },
    async load(id) {
      if (id !== 'virtual:entry') return
      signalBuildInBinding()
      await buildHeld
      return 'export default 1'
    },
  }],
})

// Read every binding-backed value up front. Once the binding is disposed its
// emnapi context is gone, so any later call into it traps the wasm module.
const target = getRuntimeCapabilities().target

// Read the binding out of the require cache rather than loading a second copy:
// only the instance the build runs on owns the work that must be drained.
const rootRequire = createRequire(nodeRequire.resolve('rolldown/package.json'))
const bindingEntry = rootRequire.resolve(${JSON.stringify(bindingPackage)})
const loadedBinding = rootRequire.cache[bindingEntry]
if (!loadedBinding) {
  throw new Error('The build did not load ' + bindingEntry)
}
const dispose = loadedBinding.exports[Symbol.for('napi.rs.wasi.dispose')]
if (typeof dispose !== 'function') {
  throw new Error(
    'The WASI binding must publish a dispose function, received ' + typeof dispose,
  )
}

// Anything crashing out of band has to fail this consumer rather than be
// swallowed, but it must not abort the run either: the report below is the
// evidence. Collect and assert on them in the driver.
const failures = []
process.on('uncaughtException', (error) => {
  failures.push('uncaughtException: ' + (error && error.stack ? error.stack : String(error)))
})
process.on('unhandledRejection', (error) => {
  failures.push('unhandledRejection: ' + (error && error.stack ? error.stack : String(error)))
})

// Record a settlement the moment it happens. Attaching the handler at creation
// is what keeps a cancelled task from being reported as an unhandled rejection
// before the report gets to read it.
const record = (promise) => {
  const entry = {}
  entry.promise = Promise.resolve(promise).then(
    (value) => {
      entry.settled = { status: 'fulfilled', value }
    },
    (reason) => {
      entry.settled = { status: 'rejected', reason }
    },
  )
  return entry
}
// REFERENCED on purpose, and only ever reached after the disposal has settled:
// the loader's own keep-alive obligation is covered by the UNREFERENCED
// watchdog below, so this cannot stand in for it. Stranded work would otherwise
// leave the child with nothing to say.
const settlementOf = (entry) => {
  let timer
  return Promise.race([
    entry.promise.then(() => {
      clearTimeout(timer)
      return entry.settled
    }),
    new Promise((resolve) => {
      timer = setTimeout(() => resolve({ status: 'unsettled' }), settleWaitMs)
    }),
  ])
}
const describe = (settlement) =>
  settlement.status !== 'rejected'
    ? settlement.status
    : 'rejected:' +
      (settlement.reason && settlement.reason.name ? settlement.reason.name : typeof settlement.reason) +
      ':' +
      String(settlement.reason && settlement.reason.message ? settlement.reason.message : settlement.reason).slice(0, 200)

const buildRecord = record(bundle.generate().then((result) => result.output.length))
await buildReachedBinding

const source = 'export const value: number = 1\\n'.repeat(400)
const workRecords = []
for (let index = 0; index < asyncWorkCount; index += 1) {
  workRecords.push(record(transform('in-flight-' + index + '.ts', source, {})))
}
// NO await between the outstanding work and the disposal: that is what makes
// every one of these still owed a completion callback when disposal starts.
const disposePromise = dispose()
releaseBuild()

// Mirror the driver's withTimeout here, but UNREFERENCED. Holding the event
// loop open across a pending disposal is the loader's own job; a referenced
// watchdog would supply that liveness and hide its loss. Without it Node simply
// exits mid-disposal, and the driver reads the missing marker as exactly that.
let disposeTimeout
let disposed = false
try {
  await Promise.race([
    Promise.resolve(disposePromise).then(() => {
      disposed = true
    }),
    new Promise((_, reject) => {
      disposeTimeout = setTimeout(
        () =>
          reject(
            new Error('The WASI disposal never settled after ' + disposeTimeoutMs + 'ms'),
          ),
        disposeTimeoutMs,
      )
      disposeTimeout.unref()
    }),
  ])
} finally {
  clearTimeout(disposeTimeout)
}

const asyncWork = (await Promise.all(workRecords.map(settlementOf))).map(describe)
const build = describe(await settlementOf(buildRecord))

const workerExitDeadline = Date.now() + workerExitWaitMs
while (countExitedWorkers() < workers.length && Date.now() < workerExitDeadline) {
  await new Promise((resolve) => setTimeout(resolve, 50))
}

// writeSync, not process.stdout.write: a stranded-work run is killed by the
// driver's timeout, and a buffered pipe write would be lost with it.
writeSync(
  1,
  JSON.stringify({
    marker: ${JSON.stringify(marker)},
    target,
    disposed,
    build,
    asyncWork,
    workers: workers.length,
    exitedWorkers: countExitedWorkers(),
    workerErrors,
    failures,
  }),
)
`;
}

function renderThreadlessBindingTypeExercise(packageName) {
  return `
import {
  BindingBundler,
  type BindingResult,
  type BindingOutputs,
} from ${JSON.stringify(packageName)}
import {
  createInstance,
  type WorkerdRolldownInstance,
} from ${JSON.stringify(`${packageName}/workerd`)}
import wasm from ${JSON.stringify(`${packageName}/wasm`)}
import wasmAlias from ${JSON.stringify(`${packageName}/wasm.wasm`)}

const constructor: typeof BindingBundler = BindingBundler
const modules: WebAssembly.Module[] = [wasm, wasmAlias]
type GenerateResult = BindingResult<BindingOutputs>
declare const result: GenerateResult
declare const instance: WorkerdRolldownInstance
void [constructor, modules, result, instance, createInstance]
`;
}

const browserPackageTypeExerciseSource = `
import { rolldown } from '@rolldown/browser'
import { getRuntimeCapabilities } from '@rolldown/browser/experimental'
import {
  createInstance,
  type WorkerdRolldownInstance,
} from '@rolldown/browser/workerd'
import wasm from '@rolldown/browser/workerd/wasm'
import wasmAlias from '@rolldown/browser/workerd/wasm.wasm'

const modules: WebAssembly.Module[] = [wasm, wasmAlias]
declare const instance: WorkerdRolldownInstance
void [modules, instance, createInstance, getRuntimeCapabilities, rolldown]
`;

const rootPackageTypeExerciseSource = `
import { rolldown } from 'rolldown'
import { createInstance, type WorkerdRolldownInstance } from 'rolldown/workerd'
import wasm from 'rolldown/wasm'
import wasmAlias from 'rolldown/wasm.wasm'

const modules: WebAssembly.Module[] = [wasm, wasmAlias]
declare const instance: WorkerdRolldownInstance
void [modules, instance, createInstance, rolldown]
`;

// A child that strands async work does not fail, it never ends: the promises
// stay pending and the emnapi waiting-request counter keeps a `MessageChannel`
// port referenced, so Node has no reason to exit. `run` kills it at
// `consumerTimeoutMs` and throws something that says nothing about why, so name
// the symptom here and hand over whatever the child managed to report first.
async function runDisposeInFlightExercise(consumerDir, filename, source) {
  try {
    return await runNodeModule(consumerDir, filename, source, [], {
      compareStdout: false,
      returnAllResults: true,
      env: {
        NAPI_RS_ENFORCE_VERSION_CHECK: '1',
        NAPI_RS_FORCE_WASI: 'error',
      },
    });
  } catch (error) {
    assert.fail(
      `${filename} never exited (${error.killed ? `killed after ${consumerTimeoutMs}ms` : error.message}), which is how a disposal that strands outstanding napi_async_work shows up: the completion callbacks never run, so the promises stay pending and emnapi keeps the process alive. Partial stdout: ${JSON.stringify(error.stdout ?? '')}`,
    );
  }
}

async function exerciseRootPackageLayouts(consumerDir, packageManager, packedFlavors) {
  const packageDirs = getInstalledOptionalPackageDirs(consumerDir, packedFlavors);
  for (const flavor of ['threaded', 'threadless']) {
    const threaded = flavor === 'threaded';
    await withOnlyOptionalFlavor(packageDirs, flavor, async () => {
      const rootResult = await runNodeModule(
        consumerDir,
        `exercise-root-${flavor}.mjs`,
        rootPackageExerciseSource,
        [],
        {
          env: {
            NAPI_RS_ENFORCE_VERSION_CHECK: '1',
            NAPI_RS_FORCE_WASI: 'error',
          },
        },
      );
      assertRootPackageExercise(rootResult.stdout, flavor);

      if (flavor === 'threaded') {
        assertThreadedDisposeExercise(
          await runNodeModule(
            consumerDir,
            'exercise-root-threaded-dispose.mjs',
            threadedDisposeExerciseSource,
            [],
            {
              compareStdout: false,
              returnAllResults: true,
              env: {
                NAPI_RS_ENFORCE_VERSION_CHECK: '1',
                NAPI_RS_FORCE_WASI: 'error',
              },
            },
          ),
        );
      }

      assertDisposeInFlightExercise(
        await runDisposeInFlightExercise(
          consumerDir,
          `exercise-root-${flavor}-dispose-in-flight.mjs`,
          renderDisposeInFlightExerciseSource({
            bindingPackage: packedFlavors.get(flavor).name,
            marker: `${flavor}-wasi-dispose-in-flight`,
          }),
        ),
        {
          marker: `${flavor}-wasi-dispose-in-flight`,
          target: threaded ? 'wasi-threads' : 'wasi',
          threaded,
        },
      );

      if (flavor === 'threadless') {
        for (const wasmSubpath of ['wasm', 'wasm.wasm']) {
          const workerdResult = await runNodeModule(
            consumerDir,
            `exercise-root-workerd-${wasmSubpath.replace('.', '-')}.mjs`,
            exerciseSource,
            ['rolldown/workerd', `rolldown/${wasmSubpath}`],
          );
          const { resolution } = assertThreadlessExercise(workerdResult.stdout);
          assertResolutionSuffix(resolution, 'workerd', `/${generatedRootFiles.workerdEntry}`);
          assertResolutionSuffix(resolution, 'wasm', `/${generatedRootFiles.wasmEntry}`);
        }
      }
    });
  }
  console.log(
    `Validated separate threaded and threadless ${packageManager} root layouts, including threaded WASI disposal after a real build and both flavors' disposal over in-flight async work`,
  );
}

try {
  runtimeNodes = await resolveRuntimeNodes();
  // Probe from the neutral temp dir: the repo root pins its own (newer)
  // packageManager, which corepack-invoked pnpm refuses to impersonate.
  assert.equal((await runPnpm(['--version'], { cwd: tempDir })).stdout.trim(), pnpmVersion);

  const packDir = path.join(tempDir, 'pack');
  const packedFlavors = new Map();
  for (const flavor of flavors) {
    const manifest = JSON.parse(await readFile(path.join(flavor.dir, 'package.json'), 'utf8'));
    assert.equal(
      manifest.version,
      rootPackageManifest.version,
      `${manifest.name} must be generated after the root release version is determined`,
    );
    assert.deepEqual(
      manifest.files,
      flavor.files,
      `${manifest.name} manifest must declare exactly its complete artifact set`,
    );
    const tarball = await pack(flavor.dir, packDir);
    assert.equal(
      manifest.dependencies?.['@oxc-project/types'],
      publicTypesVersion,
      `${manifest.name} must declare its public @oxc-project/types dependency`,
    );
    for (const dependency of runtimePackages) {
      assert.equal(
        manifest.dependencies?.[dependency],
        undefined,
        `${manifest.name} must not resolve ${dependency} from the registry`,
      );
    }
    packedFlavors.set(flavor.key, { ...flavor, manifest, name: manifest.name, tarball });
  }

  const directConsumer = path.join(tempDir, 'direct-consumer');
  await mkdir(directConsumer);
  await writeFile(
    path.join(directConsumer, 'package.json'),
    `${JSON.stringify(
      {
        name: 'rolldown-wasi-direct-consumer',
        private: true,
        type: 'module',
        packageManager: 'pnpm@11.9.0',
        dependencies: Object.fromEntries(
          [...packedFlavors.values()].map(({ name, tarball }) => [
            name,
            fileDependency(directConsumer, tarball),
          ]),
        ),
      },
      null,
      2,
    )}\n`,
  );
  await runPnpm(
    ['--pm-on-fail=ignore', 'install', '--ignore-scripts', '--config.node-linker=isolated'],
    { cwd: directConsumer },
  );
  assertNoRegistryRuntimePackages(
    await readFile(path.join(directConsumer, 'pnpm-lock.yaml'), 'utf8'),
    'direct pnpm consumer',
  );

  for (const flavor of packedFlavors.values()) {
    const installedFlavorDir = await realpath(
      path.join(directConsumer, 'node_modules', flavor.name),
    );
    flavor.installedDir = installedFlavorDir;
    for (const { name, sourceType } of flavor.loaders) {
      const code = await readFile(path.join(installedFlavorDir, name), 'utf8');
      assert.deepEqual(
        findBareRuntimeImports(code, sourceType),
        [],
        `${name} in ${flavor.name} must be self-contained`,
      );
      if (name === 'rolldown-binding.wasip1.cjs') {
        assertThreadlessNodeLifecycle(code, `${name} in ${flavor.name}`);
      }
    }
    await assertPackedNotices(installedFlavorDir, flavor.manifest, flavor.name);
  }

  const threadlessFlavor = packedFlavors.get('threadless');
  const directResult = await runNodeModule(
    directConsumer,
    'exercise-threadless.mjs',
    exerciseSource,
    [`${threadlessFlavor.name}/workerd`, `${threadlessFlavor.name}/wasm`],
  );
  const {
    runtimeExports,
    workerdExports,
    resolution: directResolution,
  } = assertThreadlessExercise(directResult.stdout);
  assertResolutionSuffix(directResolution, 'workerd', '/rolldown-binding.wasip1-deferred.js');
  assertResolutionSuffix(directResolution, 'wasm', '/rolldown-binding.wasm32-wasip1.wasm');
  await assertWorkerdDeclarationParity(
    threadlessFlavor.installedDir,
    runtimeExports,
    workerdExports,
  );

  const directBindingResult = await runNodeModule(
    directConsumer,
    'exercise-threadless-binding-root.mjs',
    directBindingExerciseSource,
    [threadlessFlavor.name],
  );
  assertThreadlessBindingExercise(directBindingResult.stdout);

  const browserBindingResult = await runNodeModule(
    directConsumer,
    'exercise-threadless-binding-browser.mjs',
    browserBindingExerciseSource,
    [threadlessFlavor.name],
    { nodeArgs: ['--conditions=browser'] },
  );
  assertThreadlessBrowserBindingExercise(browserBindingResult.stdout);

  await typecheckPackedConsumer(
    directConsumer,
    'typecheck-threadless-binding.mts',
    renderThreadlessBindingTypeExercise(threadlessFlavor.name),
  );

  const threadlessBrowserResult = await exerciseThreadlessBrowserPackage(
    threadlessFlavor.installedDir,
  );
  assert.deepEqual(threadlessBrowserResult, {
    outputs: 1,
    memories: [{ initial: threadlessInitialPages, maximum: threadlessMaximumPages, shared: false }],
    capabilities: threadlessCapabilities,
  });

  const memoryFloorResults = await runNodeModule(
    directConsumer,
    'exercise-threadless-memory-floor.mjs',
    memoryFloorExerciseSource,
    [`${threadlessFlavor.name}/workerd`, `${threadlessFlavor.name}/wasm.wasm`],
    {
      compareStdout: false,
      returnAllResults: true,
    },
  );
  for (const { stdout, runtimeVersion } of memoryFloorResults) {
    const memoryFloorResult = JSON.parse(stdout);
    assert.deepEqual(memoryFloorResult.memory, {
      initialPages: threadlessInitialPages,
      maximumPages: threadlessMaximumPages,
      pageBytes: wasmPageBytes,
      initialBytes: threadlessInitialPages * wasmPageBytes,
      maximumBytes: threadlessMaximumPages * wasmPageBytes,
    });
    assert.equal(memoryFloorResult.moduleCount, 256);
    assert.equal(memoryFloorResult.rounds, 3);
    assert.equal(memoryFloorResult.samples.length, 3);
    for (const sample of memoryFloorResult.samples) {
      assert.ok(sample.initialMemoryBytes >= 64 * 1024 * 1024);
      assert.ok(sample.initialMemoryBytes <= 65 * 1024 * 1024);
      assert.equal(sample.outputs, 1);
      assert.ok(
        sample.afterBuildMemoryBytes <= 128 * 1024 * 1024,
        `Representative workerd build exceeded 128 MiB under ${runtimeVersion}: ${sample.afterBuildMemoryBytes}`,
      );
    }
    assert.equal(memoryFloorResult.stats.liveInstances, 0);
    assert.equal(memoryFloorResult.stats.createdInstances, 3);
  }

  const threadedFlavor = packedFlavors.get('threaded');
  await exerciseEmbeddedWasiThreadsRefresh(directConsumer, threadedFlavor.installedDir);
  const threadedBrowserResult = await exerciseThreadedBrowserPackage(threadedFlavor.installedDir);
  const { workers: threadedBrowserWorkers, ...threadedBrowserDetails } = threadedBrowserResult;
  assert.ok(threadedBrowserWorkers >= 1, 'Threaded WASI browser loader did not construct a Worker');
  assert.deepEqual(threadedBrowserDetails, {
    crossOriginIsolated: true,
    sharedArrayBuffer: 'function',
    sharedMemories: 1,
    workerEventErrors: 0,
    workerMessageErrors: 0,
    outputs: 1,
    capabilities: threadedCapabilities,
  });

  const stagedBrowserDir = path.join(tempDir, 'browser-package');
  await mkdir(stagedBrowserDir);
  // This consumer validates the self-contained path: the packed dist bundles
  // its emnapi/wasm runtimes and `assertNoRegistryRuntimePackages` requires
  // that none of them install from the registry. Drop the registry runtime
  // dependencies from the staged manifest -- their workspace `catalog:`
  // specifiers cannot resolve outside the repository either, where this staged
  // copy is packed. check-workerd-packed-consumer.mjs covers the
  // with-registry-dependencies path by packing the workspace manifest in place.
  const stagedBrowserManifest = JSON.parse(
    await readFile(path.join(browserPackageDir, 'package.json'), 'utf8'),
  );
  for (const dependency of runtimePackages) {
    delete stagedBrowserManifest.dependencies?.[dependency];
  }
  if (
    stagedBrowserManifest.dependencies &&
    Object.keys(stagedBrowserManifest.dependencies).length === 0
  ) {
    delete stagedBrowserManifest.dependencies;
  }
  assert.doesNotMatch(
    JSON.stringify(stagedBrowserManifest),
    /"catalog:/,
    'Staged @rolldown/browser manifest must not retain workspace catalog specifiers',
  );
  await writeFile(
    path.join(stagedBrowserDir, 'package.json'),
    `${JSON.stringify(stagedBrowserManifest, null, 2)}\n`,
  );
  await cp(path.join(browserPackageDir, 'bin'), path.join(stagedBrowserDir, 'bin'), {
    recursive: true,
  });
  await cp(path.join(browserPackageDir, 'dist'), path.join(stagedBrowserDir, 'dist'), {
    recursive: true,
  });
  for (const notice of Object.keys(requiredNotices)) {
    await copyFile(path.join(repoRoot, notice), path.join(stagedBrowserDir, notice));
  }
  const browserManifest = JSON.parse(
    await readFile(path.join(stagedBrowserDir, 'package.json'), 'utf8'),
  );
  // Both manifests are versioned in git by bump-version.js -- nothing stamps them
  // after this check runs. That script skips only `private` workspace packages
  // (scripts/misc/bump-version.js:59), and @rolldown/browser is no longer one, so
  // both receive the pkg.pr.new canary flow's `+commit.<sha>` build metadata.
  // Build metadata carries no semver precedence, so compare the release versions:
  // the two must stay on the same release, and only that canary suffix may differ.
  const releaseVersion = (version) => version.split('+', 1)[0];
  assert.equal(
    releaseVersion(browserManifest.version),
    releaseVersion(rootPackageManifest.version),
    '@rolldown/browser must be on the root release version before packing',
  );
  const browserTarball = await pack(stagedBrowserDir, packDir);
  const browserConsumer = path.join(tempDir, 'browser-consumer');
  await mkdir(browserConsumer);
  await writeFile(
    path.join(browserConsumer, 'package.json'),
    `${JSON.stringify(
      {
        name: 'rolldown-browser-packed-consumer',
        private: true,
        packageManager: 'pnpm@11.9.0',
        dependencies: {
          [browserManifest.name]: fileDependency(browserConsumer, browserTarball),
        },
      },
      null,
      2,
    )}\n`,
  );
  await runPnpm(['--pm-on-fail=ignore', 'install', '--config.node-linker=isolated'], {
    cwd: browserConsumer,
  });
  assertNoRegistryRuntimePackages(
    await readFile(path.join(browserConsumer, 'pnpm-lock.yaml'), 'utf8'),
    'browser pnpm consumer',
  );
  const installedBrowserDir = await realpath(
    path.join(browserConsumer, 'node_modules', browserManifest.name),
  );
  await assertPackedNotices(installedBrowserDir, browserManifest, '@rolldown/browser');

  const browserRootResult = await runNodeModule(
    browserConsumer,
    'exercise-browser-root.mjs',
    browserPackageExerciseSource,
    [],
    { nodeArgs: ['--conditions=browser'] },
  );
  assertBrowserPackageExercise(browserRootResult.stdout);

  assertBrowserPackageChromiumExercise(await exerciseBrowserPackageRoot(installedBrowserDir));

  const browserWorkerdResult = await runNodeModule(
    browserConsumer,
    'exercise-browser-workerd.mjs',
    exerciseSource,
    ['@rolldown/browser/workerd', '@rolldown/browser/workerd/wasm.wasm'],
    { nodeArgs: ['--conditions=workerd'] },
  );
  const { resolution: browserWorkerdResolution } = assertThreadlessExercise(
    browserWorkerdResult.stdout,
  );
  assertResolutionSuffix(browserWorkerdResolution, 'workerd', '/dist/workerd.browser.mjs');
  assertResolutionSuffix(
    browserWorkerdResolution,
    'wasm',
    '/dist/rolldown-binding.wasm32-wasip1.wasm',
  );

  const browserConditionWorkerdResult = await runNodeModule(
    browserConsumer,
    'exercise-browser-condition-workerd.mjs',
    exerciseSource,
    ['@rolldown/browser/workerd', '@rolldown/browser/workerd/wasm'],
    { nodeArgs: ['--conditions=browser'] },
  );
  const { resolution: browserConditionWorkerdResolution } = assertThreadlessExercise(
    browserConditionWorkerdResult.stdout,
  );
  assertResolutionSuffix(browserConditionWorkerdResolution, 'workerd', '/dist/workerd.browser.mjs');
  assertResolutionSuffix(
    browserConditionWorkerdResolution,
    'wasm',
    '/dist/rolldown-binding.wasm32-wasip1.wasm',
  );

  await typecheckPackedConsumer(
    browserConsumer,
    'typecheck-browser-package.mts',
    browserPackageTypeExerciseSource,
  );

  const stagedRootDir = path.join(tempDir, 'root-package');
  await mkdir(path.join(stagedRootDir, 'bin'), { recursive: true });
  await mkdir(path.join(stagedRootDir, 'npm'), { recursive: true });
  for (const flavor of packedFlavors.values()) {
    await cp(flavor.dir, path.join(stagedRootDir, 'npm', path.basename(flavor.dir)), {
      recursive: true,
    });
  }
  await copyFile(path.join(rootPackageDir, 'bin/cli.mjs'), path.join(stagedRootDir, 'bin/cli.mjs'));
  await cp(path.join(rootPackageDir, 'dist'), path.join(stagedRootDir, 'dist'), {
    recursive: true,
  });
  for (const localWasiLoader of ['rolldown-binding.wasi.cjs', 'rolldown-binding.wasip1.cjs']) {
    assert.equal(
      existsSync(path.join(stagedRootDir, 'dist', localWasiLoader)),
      false,
      `Packed root validation must exercise optional packages, not ${localWasiLoader} from dist`,
    );
  }
  for (const notice of Object.keys(requiredNotices)) {
    await copyFile(path.join(repoRoot, notice), path.join(stagedRootDir, notice));
  }
  const stagedRootManifest = JSON.parse(
    await readFile(path.join(rootPackageDir, 'package.json'), 'utf8'),
  );
  stagedRootManifest.napi.targets = flavors.map(({ target }) => target);
  stagedRootManifest.devDependencies.rolldown = stagedRootManifest.version;
  await writeFile(
    path.join(stagedRootDir, 'package.json'),
    `${JSON.stringify(stagedRootManifest, null, 2)}\n`,
  );
  await copyFile(
    path.join(repoRoot, 'pnpm-workspace.yaml'),
    path.join(stagedRootDir, 'pnpm-workspace.yaml'),
  );
  const prePublishArgs = [
    napiCli,
    'pre-publish',
    '--cwd',
    stagedRootDir,
    '--tag-style',
    'npm',
    '--no-gh-release',
    '--skip-optional-publish',
  ];
  const partialManifestPath = path.join(stagedRootDir, 'npm', 'wasm32-wasip1', 'package.json');
  const stagedThreadlessManifest = await readFile(partialManifestPath, 'utf8');
  const partialThreadlessManifest = JSON.parse(stagedThreadlessManifest);
  partialThreadlessManifest.dependencies.buffer = '^6.0.3';
  await writeFile(partialManifestPath, `${JSON.stringify(partialThreadlessManifest, null, 2)}\n`);
  await assert.rejects(
    run(process.execPath, prePublishArgs, { cwd: repoRoot }),
    (error) => {
      assert.notEqual(error.code, 0);
      return true;
    },
    'pre-publish must reject a partial external WASI runtime dependency set',
  );
  await writeFile(partialManifestPath, stagedThreadlessManifest);
  await run(process.execPath, prePublishArgs, { cwd: repoRoot });

  const generatedRootManifest = JSON.parse(
    await readFile(path.join(stagedRootDir, 'package.json'), 'utf8'),
  );
  for (const file of Object.values(generatedRootFiles)) {
    assert.ok(
      generatedRootManifest.files?.includes(file),
      `${file} must be explicit in the generated root packlist`,
    );
    assert.ok(existsSync(path.join(stagedRootDir, file)), `pre-publish must generate ${file}`);
  }
  const expectedRootExports = {
    './workerd': {
      types: `./${generatedRootFiles.workerdTypeDef}`,
      default: `./${generatedRootFiles.workerdEntry}`,
    },
    './wasm': {
      types: `./${generatedRootFiles.wasmTypeDef}`,
      default: `./${generatedRootFiles.wasmEntry}`,
    },
    './wasm.wasm': {
      types: `./${generatedRootFiles.wasmTypeDef}`,
      default: `./${generatedRootFiles.wasmEntry}`,
    },
  };
  // pnpm replaces `exports` with `publishConfig.exports` while packing, so
  // pre-publish generates the facade subpaths only in that publish-effective
  // map and must leave the raw dev `exports` map untouched.
  assert.deepEqual(
    generatedRootManifest.exports,
    stagedRootManifest.exports,
    'pre-publish must leave the raw dev exports map untouched when publishConfig.exports is declared',
  );
  for (const [subpath, expectedExport] of Object.entries(expectedRootExports)) {
    assert.deepEqual(
      generatedRootManifest.publishConfig?.exports?.[subpath],
      expectedExport,
      `pre-publish must generate publishConfig.exports[${JSON.stringify(subpath)}]`,
    );
  }
  for (const flavor of packedFlavors.values()) {
    assert.equal(
      generatedRootManifest.optionalDependencies?.[flavor.name],
      flavor.manifest.version,
      `pre-publish must preserve ${flavor.name} as an optional root dependency`,
    );
  }
  const rootTarball = await pack(stagedRootDir, packDir);
  // pnpm's pack rewrite is what ships the generated facade subpaths.
  const packedRootManifest = JSON.parse(
    (await run('tar', ['-xzOf', rootTarball, 'package/package.json'])).stdout,
  );
  assert.deepEqual(
    packedRootManifest.exports,
    generatedRootManifest.publishConfig.exports,
    'packed root manifest must ship publishConfig.exports as its exports map',
  );

  const rootConsumer = path.join(tempDir, 'root-consumer');
  await mkdir(rootConsumer);
  await writeFile(
    path.join(rootConsumer, 'package.json'),
    `${JSON.stringify(
      {
        name: 'rolldown-wasi-root-consumer',
        private: true,
        type: 'module',
        packageManager: 'pnpm@11.9.0',
        dependencies: {
          rolldown: fileDependency(rootConsumer, rootTarball),
        },
      },
      null,
      2,
    )}\n`,
  );
  await writeFile(
    path.join(rootConsumer, 'pnpm-workspace.yaml'),
    `overrides:\n${[...packedFlavors.values()]
      .map(
        ({ name, tarball }) =>
          `  ${JSON.stringify(name)}: ${JSON.stringify(fileDependency(rootConsumer, tarball))}`,
      )
      .join('\n')}\n`,
  );
  await runPnpm(
    ['--pm-on-fail=ignore', 'install', '--ignore-scripts', '--config.node-linker=isolated'],
    { cwd: rootConsumer },
  );
  assertNoRegistryRuntimePackages(
    await readFile(path.join(rootConsumer, 'pnpm-lock.yaml'), 'utf8'),
    'root pnpm consumer',
  );
  const installedRootDir = await realpath(path.join(rootConsumer, 'node_modules', 'rolldown'));
  const installedRootManifest = JSON.parse(
    await readFile(path.join(installedRootDir, 'package.json'), 'utf8'),
  );
  await assertPackedNotices(installedRootDir, installedRootManifest, installedRootManifest.name);
  for (const flavor of packedFlavors.values()) {
    assert.equal(
      existsSync(path.join(rootConsumer, 'node_modules', flavor.name)),
      false,
      `The root test must resolve ${flavor.name} transitively under isolated pnpm layout`,
    );
  }

  await typecheckPackedConsumer(
    rootConsumer,
    'typecheck-root-package.mts',
    rootPackageTypeExerciseSource,
  );
  await exerciseRootPackageLayouts(rootConsumer, 'pnpm', packedFlavors);

  const npmConsumer = path.join(tempDir, 'npm-root-consumer');
  await mkdir(npmConsumer);
  await writeFile(
    path.join(npmConsumer, 'package.json'),
    `${JSON.stringify(
      {
        name: 'rolldown-wasi-npm-root-consumer',
        private: true,
        type: 'module',
        dependencies: Object.fromEntries([
          ['rolldown', fileDependency(npmConsumer, rootTarball)],
          ...[...packedFlavors.values()].map(({ name, tarball }) => [
            name,
            fileDependency(npmConsumer, tarball),
          ]),
        ]),
      },
      null,
      2,
    )}\n`,
  );
  await run('npm', ['install', '--ignore-scripts', '--no-audit', '--no-fund'], {
    cwd: npmConsumer,
  });
  const npmLockfileText = await readFile(path.join(npmConsumer, 'package-lock.json'), 'utf8');
  const npmLockfile = JSON.parse(npmLockfileText);
  assertNoRegistryRuntimePackages(npmLockfileText, 'root npm consumer');
  for (const flavor of packedFlavors.values()) {
    assert.match(
      npmLockfile.packages?.[`node_modules/${flavor.name}`]?.resolved ?? '',
      /^file:/,
      `npm must satisfy ${flavor.name} from the packed local flavor`,
    );
  }
  assert.equal(
    existsSync(path.join(npmConsumer, 'node_modules/@emnapi')),
    false,
    'npm must not install a registry @emnapi scope',
  );
  assert.equal(
    existsSync(path.join(npmConsumer, 'node_modules/@napi-rs/wasm-runtime')),
    false,
    'npm must not install registry @napi-rs/wasm-runtime',
  );
  assert.equal(
    existsSync(path.join(npmConsumer, 'node_modules/buffer')),
    false,
    'npm must not install registry buffer',
  );
  await exerciseRootPackageLayouts(npmConsumer, 'npm', packedFlavors);

  const yarnConsumer = path.join(tempDir, 'yarn-root-consumer');
  await mkdir(yarnConsumer);
  const yarnFlavorDependencies = Object.fromEntries(
    [...packedFlavors.values()].map(({ name, tarball }) => [
      name,
      fileDependency(yarnConsumer, tarball),
    ]),
  );
  await writeFile(
    path.join(yarnConsumer, 'package.json'),
    `${JSON.stringify(
      {
        name: 'rolldown-wasi-yarn-root-consumer',
        private: true,
        type: 'module',
        packageManager: 'yarn@1.22.22',
        dependencies: {
          rolldown: fileDependency(yarnConsumer, rootTarball),
          ...yarnFlavorDependencies,
        },
        resolutions: yarnFlavorDependencies,
      },
      null,
      2,
    )}\n`,
  );
  assert.equal((await runYarn(['--version'], { cwd: yarnConsumer })).stdout.trim(), yarnVersion);
  await runYarn(['install', '--ignore-scripts', '--non-interactive', '--no-progress'], {
    cwd: yarnConsumer,
  });
  assertNoRegistryRuntimePackages(
    await readFile(path.join(yarnConsumer, 'yarn.lock'), 'utf8'),
    'root Yarn consumer',
  );
  for (const flavor of packedFlavors.values()) {
    assert.ok(
      existsSync(path.join(yarnConsumer, 'node_modules', flavor.name)),
      `Yarn must install packed local flavor ${flavor.name}`,
    );
  }
  assert.equal(
    existsSync(path.join(yarnConsumer, 'node_modules/@emnapi')),
    false,
    'Yarn must not install a registry @emnapi scope',
  );
  assert.equal(
    existsSync(path.join(yarnConsumer, 'node_modules/@napi-rs/wasm-runtime')),
    false,
    'Yarn must not install registry @napi-rs/wasm-runtime',
  );
  assert.equal(
    existsSync(path.join(yarnConsumer, 'node_modules/buffer')),
    false,
    'Yarn must not install registry buffer',
  );
  await exerciseRootPackageLayouts(yarnConsumer, 'Yarn', packedFlavors);

  console.log(
    `OK: packed browser/root/workerd/wasm exports, threaded browser workers, managed facades, and separate threaded/threadless root layouts execute across pnpm, npm, and Yarn consumers on ${runtimeNodes.map(({ version }) => version).join(' and ')}`,
  );
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
