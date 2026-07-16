import assert from 'node:assert/strict';
import { AsyncLocalStorage } from 'node:async_hooks';
import { execFile } from 'node:child_process';
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { parse } from 'acorn';
import { Miniflare } from 'miniflare';

const execFileAsync = promisify(execFile);
const repoRoot = fileURLToPath(new URL('../../', import.meta.url));
const browserPackageDir = path.join(repoRoot, 'packages/browser');
const wranglerPackageDir = path.dirname(
  fileURLToPath(import.meta.resolve('wrangler/package.json')),
);
const wranglerBin = path.join(wranglerPackageDir, 'bin/wrangler.js');
const compatibilityDate = '2026-06-01';
const pnpm10Version = '10.28.1';
const pnpm11Version = '11.9.0';
const tempDir = await mkdtemp(path.join(tmpdir(), 'rolldown-workerd-consumer-'));
const bundledRuntimePackages = [
  '@emnapi/core',
  '@emnapi/runtime',
  '@napi-rs/wasm-runtime',
  'buffer',
  'node:buffer',
];

async function run(command, args, options = {}) {
  return execFileAsync(command, args, {
    maxBuffer: 20 * 1024 * 1024,
    ...options,
    env: {
      ...process.env,
      COREPACK_ENABLE_DOWNLOAD_PROMPT: '0',
      WRANGLER_SEND_METRICS: 'false',
      ...options.env,
    },
  });
}

function fileDependency(fromDir, tarball) {
  return `file:${path.relative(fromDir, tarball).split(path.sep).join('/')}`;
}

async function assertInstallableWithPnpm(tarball, version) {
  const consumerDir = path.join(tempDir, `install-pnpm-${version}`);
  await mkdir(consumerDir, { recursive: true });
  await writeFile(
    path.join(consumerDir, 'package.json'),
    `${JSON.stringify(
      {
        name: `rolldown-browser-install-pnpm-${version}`,
        private: true,
        packageManager: `pnpm@${version}`,
        dependencies: {
          '@rolldown/browser': fileDependency(consumerDir, tarball),
        },
      },
      null,
      2,
    )}\n`,
  );
  await run(
    'corepack',
    [`pnpm@${version}`, 'install', '--prefer-offline', '--config.node-linker=isolated'],
    { cwd: consumerDir },
  );
  const manifest = JSON.parse(
    await readFile(path.join(consumerDir, 'node_modules/@rolldown/browser/package.json'), 'utf8'),
  );
  assert.equal(
    manifest.scripts?.preinstall,
    undefined,
    `Published @rolldown/browser must not enforce a package manager under pnpm ${version}`,
  );
}

function findBareRuntimeImports(code, sourceType) {
  const program = parse(code, { ecmaVersion: 'latest', sourceType });
  const imports = [];
  const pending = [program];

  while (pending.length > 0) {
    const node = pending.pop();
    if (!node || typeof node !== 'object') continue;

    if (
      node.type === 'ImportDeclaration' &&
      typeof node.source?.value === 'string' &&
      bundledRuntimePackages.some(
        (specifier) =>
          node.source.value === specifier || node.source.value.startsWith(`${specifier}/`),
      )
    ) {
      imports.push(node.source.value);
    }
    if (
      node.type === 'ImportExpression' &&
      typeof node.source?.value === 'string' &&
      bundledRuntimePackages.some(
        (specifier) =>
          node.source.value === specifier || node.source.value.startsWith(`${specifier}/`),
      )
    ) {
      imports.push(node.source.value);
    }
    if (
      node.type === 'CallExpression' &&
      node.arguments?.length === 1 &&
      typeof node.arguments[0]?.value === 'string' &&
      bundledRuntimePackages.some(
        (specifier) =>
          node.arguments[0].value === specifier ||
          node.arguments[0].value.startsWith(`${specifier}/`),
      ) &&
      ((node.callee?.type === 'Identifier' && node.callee.name === 'require') ||
        (node.callee?.type === 'MemberExpression' &&
          node.callee.object?.type === 'Identifier' &&
          node.callee.object.name === 'require' &&
          node.callee.property?.type === 'Identifier' &&
          node.callee.property.name === 'resolve'))
    ) {
      imports.push(node.arguments[0].value);
    }

    for (const value of Object.values(node)) {
      if (Array.isArray(value)) {
        pending.push(...value);
      } else if (value && typeof value === 'object') {
        pending.push(value);
      }
    }
  }

  return imports.sort((a, b) => a.localeCompare(b));
}

assert.deepEqual(
  findBareRuntimeImports(
    "import('@emnapi/core'); import 'buffer'; require.resolve('@napi-rs/wasm-runtime');",
    'module',
  ),
  ['@emnapi/core', '@napi-rs/wasm-runtime', 'buffer'],
  'runtime import scan must cover dynamic imports and require.resolve',
);

try {
  const packDir = path.join(tempDir, 'pack');
  const consumerDir = path.join(tempDir, 'consumer');
  const sourceDir = path.join(consumerDir, 'src');
  const outputDir = path.join(consumerDir, 'dist');
  await mkdir(packDir, { recursive: true });
  await mkdir(sourceDir, { recursive: true });

  await run('vp', ['pm', 'pack', '--pack-destination', packDir], { cwd: browserPackageDir });
  const tarballs = (await readdir(packDir)).filter((entry) => entry.endsWith('.tgz'));
  assert.equal(tarballs.length, 1, `Expected one browser-package tarball, found ${tarballs}`);
  const tarball = path.join(packDir, tarballs[0]);

  await writeFile(
    path.join(consumerDir, 'package.json'),
    `${JSON.stringify(
      {
        name: 'rolldown-workerd-packed-consumer',
        private: true,
        type: 'module',
        packageManager: `pnpm@${pnpm11Version}`,
        dependencies: {
          '@rolldown/browser': fileDependency(consumerDir, tarball),
          buffer: '6.0.3',
        },
      },
      null,
      2,
    )}\n`,
  );
  await writeFile(
    path.join(consumerDir, 'wrangler.jsonc'),
    `${JSON.stringify(
      {
        name: 'rolldown-workerd-packed-consumer',
        main: 'src/index.js',
        compatibility_date: compatibilityDate,
        rules: [
          {
            type: 'CompiledWasm',
            globs: ['**/*.wasm'],
            fallthrough: true,
          },
        ],
      },
      null,
      2,
    )}\n`,
  );
  await writeFile(
    path.join(sourceDir, 'index.js'),
    `import { createInstance } from '@rolldown/browser/workerd'
import rolldownWasm from '@rolldown/browser/workerd/wasm.wasm'
import { Buffer as ConsumerBuffer } from 'buffer'

class ConsumerBytes extends Uint8Array {}

export default {
  async fetch() {
    const instance = await createInstance(rolldownWasm)
    try {
      const bundler = new instance.exports.BindingBundler()
      const inputSources = {
        'buffer.bin': ConsumerBuffer.from([0, 1, 255]),
        'subclass.bin': new ConsumerBytes([2, 3, 4]),
      }
      try {
        const result = await bundler.generate({
          inputOptions: {
            input: [{ import: 'virtual:entry' }],
            plugins: [{
              name: 'binary-asset',
              hookUsage: 11,
              buildStart(ctx) {
                for (const [fileName, source] of Object.entries(inputSources)) {
                  ctx.emitFile({
                    fileName,
                    source: { inner: source },
                  })
                }
              },
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
          outputOptions: {
            format: 'es',
            plugins: [],
          },
        })
        if ('isBindingErrors' in result) {
          throw new Error(JSON.stringify(result.errors))
        }
        const assets = Object.fromEntries(result.assets.map((asset) => {
          const source = asset.getSource().inner
          return [asset.getFileName(), {
            sourceType: source.constructor.name,
            isView: ArrayBuffer.isView(source),
            bytes: Array.from(source),
          }]
        }))
        return new Response(JSON.stringify({
          bufferGlobal: typeof globalThis.Buffer,
          inputSources: Object.fromEntries(
            Object.entries(inputSources).map(([fileName, source]) => [
              fileName,
              {
                sourceType: source.constructor.name,
                isView: ArrayBuffer.isView(source),
              },
            ]),
          ),
          assets,
          capabilities: instance.exports.getRuntimeCapabilities(),
        }))
      } finally {
        await Promise.all([bundler.close(), bundler.close()])
      }
    } finally {
      instance.dispose()
    }
  },
}
`,
  );

  await assertInstallableWithPnpm(tarball, pnpm10Version);
  await run(
    'corepack',
    [`pnpm@${pnpm11Version}`, 'install', '--prefer-offline', '--config.node-linker=isolated'],
    { cwd: consumerDir },
  );

  const installedBrowserDir = path.join(consumerDir, 'node_modules/@rolldown/browser');
  const installedManifest = JSON.parse(
    await readFile(path.join(installedBrowserDir, 'package.json'), 'utf8'),
  );
  assert.equal(
    installedManifest.scripts?.preinstall,
    undefined,
    'Published @rolldown/browser must not enforce the repository package manager',
  );
  assert.equal(
    installedManifest.exports?.['.']?.types,
    './dist/index.d.mts',
    'Published browser root must expose its declarations under conditional exports',
  );
  for (const dependency of bundledRuntimePackages) {
    assert.equal(
      installedManifest.dependencies?.[dependency],
      undefined,
      `Published consumers must not resolve ${dependency} from the registry`,
    );
  }

  const publishedRootEntries = [
    {
      condition: 'browser',
      entry: installedManifest.exports['.'].browser,
      loader: 'rolldown-binding.wasip1-browser.js',
      sourceType: 'module',
    },
    {
      condition: 'default',
      entry: installedManifest.exports['.'].default,
      loader: 'rolldown-binding.wasip1.cjs',
      sourceType: 'script',
    },
  ];
  for (const { condition, entry, loader, sourceType } of publishedRootEntries) {
    const entryCode = await readFile(path.join(installedBrowserDir, entry), 'utf8');
    assert.ok(
      entryCode.includes(`"./${loader}"`),
      `${condition} package root does not resolve through ${loader}`,
    );

    const loaderCode = await readFile(path.join(installedBrowserDir, 'dist', loader), 'utf8');
    assert.deepEqual(
      findBareRuntimeImports(loaderCode, sourceType),
      [],
      `${loader} must vendor its emnapi/wasm runtime`,
    );
  }

  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (input, init) => {
    const url = input instanceof Request ? input.url : String(input);
    if (url.startsWith('file:')) {
      return new Response(await readFile(fileURLToPath(url)));
    }
    return originalFetch(input, init);
  };
  try {
    const browserApi = await import(
      `${pathToFileURL(path.join(installedBrowserDir, 'dist/index.browser.mjs')).href}?browser-queue`
    );
    const experimentalApi = await import(
      `${
        pathToFileURL(path.join(installedBrowserDir, 'dist/experimental-index.browser.mjs')).href
      }?browser-queue`
    );
    experimentalApi.configureAsyncContext({
      createStorage: () => new AsyncLocalStorage(),
    });
    const originalProcess = globalThis.process;
    globalThis.process = undefined;
    try {
      let buildStarts = 0;
      let markFirstBuildStarted;
      const firstBuildStarted = new Promise((resolve) => {
        markFirstBuildStarted = resolve;
      });
      let releaseFirstBuild;
      const firstBuildRelease = new Promise((resolve) => {
        releaseFirstBuild = resolve;
      });
      const browserBundle = await browserApi.rolldown({
        input: 'virtual:entry',
        plugins: [
          {
            name: 'browser-concurrent-queue',
            async buildStart() {
              buildStarts += 1;
              if (buildStarts === 1) {
                markFirstBuildStarted();
                await firstBuildRelease;
              }
            },
            resolveId(id) {
              if (id === 'virtual:entry') return id;
            },
            load(id) {
              if (id === 'virtual:entry') return 'export default 1';
            },
          },
        ],
      });
      try {
        const firstBuild = browserBundle.generate();
        await firstBuildStarted;
        const secondBuild = browserBundle.generate();
        releaseFirstBuild();
        await Promise.all([firstBuild, secondBuild]);
        assert.equal(buildStarts, 2, 'Browser builds must preserve external concurrent queueing');
      } finally {
        await browserBundle.close();
      }
    } finally {
      globalThis.process = originalProcess;
    }
  } finally {
    globalThis.fetch = originalFetch;
  }

  for (const entry of ['workerd.mjs', 'workerd.browser.mjs']) {
    const workerdBundle = await readFile(path.join(installedBrowserDir, 'dist', entry), 'utf8');
    assert.deepEqual(
      findBareRuntimeImports(workerdBundle, 'module'),
      [],
      `${entry} must not import external emnapi/wasm runtime packages`,
    );
    assert.match(workerdBundle, /getCurrentThreadTaskHostContractVersion/);
    assert.match(workerdBundle, /isCurrentThreadHostRegistrationActive/);
    assert.match(workerdBundle, /reserveCurrentThreadHostRegistration/);
    assert.match(workerdBundle, /registerCurrentThreadTaskHost/);
    assert.match(workerdBundle, /unregisterCurrentThreadTaskHost/);
    assert.match(workerdBundle, /__actualVersion !== 4/);
    assert.match(workerdBundle, /Reflect\.apply\(__reserve, __binding, \[\]\)/);
    assert.match(workerdBundle, /Reflect\.apply\(__register, __binding, __registration\)/);
    assert.match(workerdBundle, /Reflect\.apply\(__unregister, __binding, __registration\)/);
    assert.doesNotMatch(
      workerdBundle,
      /driveCurrentThreadRuntimeTasks|cancelCurrentThreadRuntimeTaskDispatch|dispatchHigh|dispatchLow/,
    );
    assert.match(workerdBundle, /clearTimeout/);
    assert.match(workerdBundle, /timer\.resolve\(\)/);
  }

  const defaultWorkerd = await import(
    pathToFileURL(path.join(installedBrowserDir, 'dist/workerd.mjs')).href
  );
  const browserWorkerd = await import(
    pathToFileURL(path.join(installedBrowserDir, 'dist/workerd.browser.mjs')).href
  );
  const workerdModule = await WebAssembly.compile(
    await readFile(path.join(installedBrowserDir, 'dist/rolldown-binding.wasm32-wasip1.wasm')),
  );
  assert.equal(defaultWorkerd.instantiate, defaultWorkerd.createInstance);
  assert.equal(browserWorkerd.instantiate, browserWorkerd.createInstance);
  for (const workerdEntry of [defaultWorkerd, browserWorkerd]) {
    for (const privateExport of [
      'getDeferredInstanceBinding',
      'cancelCurrentThreadRuntimeTaskDispatch',
      'driveCurrentThreadRuntimeTasks',
      'isCurrentThreadHostRegistrationActive',
      'registerCurrentThreadTaskHost',
      'registerTimerHost',
      'reserveCurrentThreadHostRegistration',
      'unregisterCurrentThreadTaskHost',
      'unregisterTimerHost',
    ]) {
      assert.equal(
        privateExport in workerdEntry,
        false,
        `Managed workerd package entry must not expose ${privateExport}`,
      );
    }
  }
  const callerMemory = new WebAssembly.Memory({
    initial: defaultWorkerd.WORKERD_WASM_MEMORY.initialPages,
    maximum: defaultWorkerd.WORKERD_WASM_MEMORY.maximumPages,
  });
  let firstInstance;
  let unsafeSecondInstance;
  let retainedCapabilities;
  let RetainedBundler;
  let firstBundler;
  try {
    firstInstance = await defaultWorkerd.createInstance(workerdModule, { memory: callerMemory });
    retainedCapabilities = firstInstance.exports.getRuntimeCapabilities;
    RetainedBundler = firstInstance.exports.BindingBundler;
    firstBundler = new RetainedBundler();
    assert.throws(
      () => Reflect.set(firstBundler, 'close', async () => {}),
      /Cannot replace or remove close/,
    );
    assert.throws(
      () => Reflect.set(Object.getPrototypeOf(firstBundler), 'close', async () => {}),
      /Cannot replace or remove close/,
    );
    for (const privateExport of [
      'cancelCurrentThreadRuntimeTaskDispatch',
      'driveCurrentThreadRuntimeTasks',
      'isCurrentThreadHostRegistrationActive',
      'registerCurrentThreadTaskHost',
      'registerTimerHost',
      'reserveCurrentThreadHostRegistration',
      'unregisterCurrentThreadTaskHost',
      'unregisterTimerHost',
    ]) {
      assert.equal(
        privateExport in firstInstance.exports,
        false,
        `Managed workerd binding exports must hide ${privateExport}`,
      );
    }
    const [secondAttempt] = await Promise.allSettled([
      browserWorkerd.createInstance(workerdModule, { memory: callerMemory }),
    ]);
    if (secondAttempt.status === 'fulfilled') {
      unsafeSecondInstance = secondAttempt.value;
      assert.fail('Independent workerd bundles reused the same caller-provided memory');
    }
    assert.match(secondAttempt.reason.message, /initialization attempt/);
  } finally {
    try {
      await firstBundler?.close();
    } finally {
      unsafeSecondInstance?.dispose();
      firstInstance?.dispose();
    }
  }
  assert.throws(() => retainedCapabilities(), /This workerd Rolldown instance has been disposed/);
  assert.throws(() => new RetainedBundler(), /This workerd Rolldown instance has been disposed/);

  await run(
    process.execPath,
    [
      '--input-type=module',
      '--eval',
      `
        import assert from 'node:assert/strict'
        import { readFile } from 'node:fs/promises'

        const workerd = await import(${JSON.stringify(
          pathToFileURL(path.join(installedBrowserDir, 'dist/workerd.mjs')).href,
        )})
        const module = await WebAssembly.compile(
          await readFile(${JSON.stringify(
            path.join(installedBrowserDir, 'dist/rolldown-binding.wasm32-wasip1.wasm'),
          )}),
        )
        const memory = new WebAssembly.Memory({
          initial: workerd.WORKERD_WASM_MEMORY.initialPages,
          maximum: workerd.WORKERD_WASM_MEMORY.maximumPages,
        })
        Object.preventExtensions(globalThis)
        assert.equal(workerd.instantiate, workerd.createInstance)
        const instance = await workerd.createInstance(module, { memory })
        instance.dispose()
        assert.equal(instance.disposed, true)
      `,
    ],
    { cwd: consumerDir },
  );

  await run(
    process.execPath,
    [wranglerBin, 'deploy', '--dry-run', '--experimental-autoconfig=false', '--outdir', outputDir],
    { cwd: consumerDir },
  );

  const outputEntries = await readdir(outputDir);
  const workerScripts = outputEntries.filter((entry) => entry.endsWith('.js'));
  const wasmModules = outputEntries.filter((entry) => entry.endsWith('.wasm'));
  assert.equal(workerScripts.length, 1, `Expected one bundled Worker, found ${workerScripts}`);
  assert.equal(wasmModules.length, 1, `Expected one CompiledWasm module, found ${wasmModules}`);

  const miniflare = new Miniflare({
    compatibilityDate,
    modules: true,
    modulesRoot: outputDir,
    scriptPath: path.join(outputDir, workerScripts[0]),
    modulesRules: [
      {
        type: 'CompiledWasm',
        include: ['**/*.wasm'],
        fallthrough: true,
      },
    ],
  });
  try {
    const response = await miniflare.dispatchFetch('http://localhost/');
    const body = await response.text();
    assert.equal(response.status, 200, body);
    const payload = JSON.parse(body);
    assert.equal(payload.bufferGlobal, 'undefined');
    assert.deepEqual(payload.inputSources, {
      'buffer.bin': {
        sourceType: 'Buffer',
        isView: true,
      },
      'subclass.bin': {
        sourceType: 'ConsumerBytes',
        isView: true,
      },
    });
    assert.deepEqual(payload.assets, {
      'buffer.bin': {
        sourceType: 'Uint8Array',
        isView: true,
        bytes: [0, 1, 255],
      },
      'subclass.bin': {
        sourceType: 'Uint8Array',
        isView: true,
        bytes: [2, 3, 4],
      },
    });
    assert.equal(payload.capabilities.backend, 'shared');
    assert.equal(payload.capabilities.flavor, 'CurrentThread');
    assert.equal(payload.capabilities.target, 'wasi');
    assert.equal(payload.capabilities.wasi, true);
    assert.equal(payload.capabilities.asyncRuntimeBuild, true);
    assert.equal(payload.capabilities.threads, false);
    assert.equal(payload.capabilities.timers, true);
    assert.equal(payload.capabilities.watchSupported, false);
    assert.equal(payload.capabilities.blockOnJsThreadSafe, false);
  } finally {
    await miniflare.dispose();
  }

  console.log(
    'OK: packed @rolldown/browser vendors its runtimes, rejects cross-bundle memory reuse, bundles with Wrangler, and runs in Miniflare',
  );
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
