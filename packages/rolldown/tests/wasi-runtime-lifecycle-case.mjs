import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { Worker } from 'node:worker_threads';

const [rolldownApi, experimentalApi] = await withTimeout(
  Promise.all([import('rolldown'), import('rolldown/experimental')]),
  60_000,
  'the threaded-WASI package did not finish loading',
);
const { build, rolldown, watch } = rolldownApi;
const { defineParallelPlugin, dev, getAsyncRuntimeConfig, getRuntimeCapabilities, scan } =
  experimentalApi;

const require = createRequire(import.meta.url);
const packageDir = path.dirname(require.resolve('rolldown/package.json'));
const distDir = path.join(packageDir, 'dist');
const bindingPath = path.join(distDir, 'rolldown-binding.wasi.cjs');
const binding = require(bindingPath);
const completed = [];

const runtimeCapabilities = getRuntimeCapabilities();
const runtimeConfig = getAsyncRuntimeConfig();

assert.equal(
  runtimeCapabilities.target,
  'wasi-threads',
  'the WASI lifecycle suite must run against the threaded-WASI artifact',
);
assert.equal(
  runtimeCapabilities.backend,
  'shared',
  'the published threaded-WASI artifact must use the shared tokio-free scheduler',
);
assert.equal(
  runtimeCapabilities.asyncRuntimeBuild,
  true,
  'the published threaded-WASI artifact is built on the shared scheduler',
);
// The shared scheduler has no MultiThread executor on WebAssembly (it is
// Rayon-backed and Rayon is not compiled for wasm), so the resolver normalizes
// every non-native target to CurrentThread. The real OS threads of
// `wasm32-wasip1-threads` change the loader, not the executor.
assert.equal(
  runtimeCapabilities.flavor,
  'CurrentThread',
  'every WebAssembly artifact resolves to the shared scheduler CurrentThread flavor',
);
assert.equal(
  runtimeCapabilities.threads,
  false,
  'the threaded-WASI artifact schedules on the calling lane only',
);
assert.equal(
  runtimeCapabilities.devSupported,
  false,
  'dev needs a MultiThread executor, which no WebAssembly artifact has',
);
// The lease API is a compatibility no-op on the shared runtime (the runtime
// lifecycle follows the N-API environment hooks), but the generated WASI
// loaders still acquire a lease at import and release it at teardown, so the
// export has to stay.
assert.equal(
  typeof binding.acquireAsyncRuntime,
  'function',
  'the generated threaded-WASI binding must export acquireAsyncRuntime',
);
assert.deepEqual(
  Object.keys(runtimeConfig).sort(),
  ['drainLingerUs', 'flavor', 'maxBlockingTasks', 'workerThreads'],
  'the threaded-WASI runtime config surface must not drift',
);
assert.deepEqual(
  {
    flavor: runtimeConfig.flavor,
    maxBlockingTasks: runtimeConfig.maxBlockingTasks,
    workerThreads: runtimeConfig.workerThreads,
  },
  {
    flavor: 'CurrentThread',
    maxBlockingTasks: 1,
    workerThreads: 1,
  },
  'the threaded-WASI artifact must report the shared scheduler single-lane shape',
);
assert.equal(
  typeof runtimeConfig.drainLingerUs,
  'number',
  'the drainer linger budget is reported for introspection parity',
);

const poolCapProbe = spawnSync(
  process.execPath,
  [
    '--input-type=module',
    '-e',
    `
      const { getAsyncRuntimeConfig, getRuntimeCapabilities } =
        await import('rolldown/experimental');
      console.log(JSON.stringify({
        capabilities: getRuntimeCapabilities(),
        config: getAsyncRuntimeConfig(),
      }));
      process.exit(0);
    `,
  ],
  {
    cwd: path.dirname(fileURLToPath(import.meta.url)),
    encoding: 'utf8',
    env: {
      ...process.env,
      NAPI_RS_ASYNC_WORK_POOL_SIZE: '2048',
    },
    timeout: 60_000,
  },
);
assert.equal(poolCapProbe.error, undefined, poolCapProbe.stderr);
assert.equal(poolCapProbe.status, 0, poolCapProbe.stderr || poolCapProbe.stdout);
// `NAPI_RS_ASYNC_WORK_POOL_SIZE` sized napi-rs' Tokio work pool, which this
// artifact no longer has. The probe therefore now guards the opposite
// contract: a stray Tokio-era pool variable must NOT resize the shared
// scheduler, and the artifact keeps its one-lane CurrentThread shape.
const poolCapReport = JSON.parse(poolCapProbe.stdout.trim().split('\n').at(-1));
assert.equal(
  typeof poolCapReport.config.drainLingerUs,
  'number',
  'the probed runtime config must report the drainer linger budget',
);
delete poolCapReport.config.drainLingerUs;
assert.deepEqual(poolCapReport, {
  capabilities: {
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: false,
    flavor: 'CurrentThread',
    target: 'wasi-threads',
    threads: false,
    timers: true,
    wasi: true,
    watchSupported: false,
  },
  config: {
    flavor: 'CurrentThread',
    maxBlockingTasks: 1,
    workerThreads: 1,
  },
});

await check('worker loader retries rejected inherited execArgv', () => {
  const probe = spawnSync(
    process.execPath,
    [
      '--title=rolldown-wasi-worker-probe',
      '--stack-trace-limit=50',
      '--trace-warnings',
      '--input-type=module',
      '--eval',
      `
        const { getRuntimeCapabilities } = await import('rolldown/experimental');
        console.log(JSON.stringify(getRuntimeCapabilities()));
      `,
    ],
    {
      cwd: path.dirname(fileURLToPath(import.meta.url)),
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 60_000,
    },
  );

  assert.equal(probe.error, undefined, probe.stderr);
  assert.equal(probe.status, 0, probe.stderr || probe.stdout);
  assert.equal(JSON.parse(probe.stdout.trim().split('\n').at(-1)).target, 'wasi-threads');
});

await check('loader cleanup settles pending work and supports same-realm reload', () => {
  const probe = spawnSync(
    process.execPath,
    [fileURLToPath(new URL('./wasi-loader-context-lifecycle.mjs', import.meta.url))],
    {
      cwd: path.dirname(fileURLToPath(import.meta.url)),
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 60_000,
    },
  );

  assert.equal(probe.error, undefined, probe.stderr);
  assert.equal(probe.status, 0, probe.stderr || probe.stdout);
  assert.match(probe.stdout, /WASI loader context cleanup and reload completed/);
});

await check('watch fails before setup and remains closable', async () => {
  let optionsHookCalls = 0;
  const watcher = watch({
    input: 'virtual:unsupported-watch',
    plugins: [
      {
        name: 'unsupported-watch',
        options(options) {
          optionsHookCalls += 1;
          return options;
        },
      },
    ],
  });
  const events = [];
  let reportedError;
  const ended = new Promise((resolve) => {
    watcher.on('event', (event) => {
      events.push(event.code);
      if (event.code === 'ERROR') {
        reportedError = event.error;
      } else if (event.code === 'END') {
        resolve();
      }
    });
  });

  await Promise.all([ended, watcher.close()]);
  assert.deepEqual(events, ['ERROR', 'END']);
  assert.equal(reportedError?.code, 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE');
  assert.equal(reportedError?.feature, 'watch');
  assert.equal(optionsHookCalls, 0);
  await watcher.close();
});

await check('overlapping owners and restart after final release', async () => {
  const [first, second] = await Promise.all([
    createVirtualBundle('overlap-first'),
    createVirtualBundle('overlap-second'),
  ]);
  try {
    await Promise.all([first.generate(), second.generate()]);
    await first.close();
    const output = await second.generate();
    assert.match(output.output[0].code, /overlap-second/);
  } finally {
    await Promise.allSettled([first.close(), second.close()]);
  }

  await generateAndClose('restart-after-overlap');
});

// Tokio retired a refcounted runtime between the last release and the next
// acquisition, so a tight reacquire loop used to race that retirement. The
// shared runtime's lifecycle follows the N-API environment instead and the
// leases are compatibility no-ops, so the loop now guards the weaker but still
// real contract: churning leases must not disturb a later build.
await check('rapid lease churn leaves the shared runtime usable', async () => {
  for (let iteration = 0; iteration < 24; iteration += 1) {
    const lease = await binding.acquireAsyncRuntime();
    lease.release();
  }

  await generateAndClose('restart-after-lease-churn');
});

// The original case asserted that a worker's acquisition stayed PENDING behind
// Tokio's retirement barrier. That barrier no longer exists -- a lease is a
// no-op -- and the assertion only survived because one acquisition round-trip
// through the wasm worker proxy measures ~33ms against its 25ms window, i.e. it
// asserted machine speed, not runtime behaviour. What still matters, and is
// what #8411/#8747 were about, is that tearing an environment down mid
// acquisition must not wedge the runtime for the surviving realm.
await check('environment teardown mid-acquisition leaves the main realm usable', async () => {
  const worker = new Worker(
    `
      const { parentPort } = require('node:worker_threads');
      const binding = require(${JSON.stringify(bindingPath)});
      parentPort.postMessage({ type: 'ready' });
      parentPort.once('message', async (message) => {
        if (message !== 'acquire') return;
        parentPort.postMessage({ type: 'acquiring' });
        try {
          const lease = await binding.acquireAsyncRuntime();
          parentPort.postMessage({ type: 'acquired' });
          lease.release();
        } catch (error) {
          parentPort.postMessage({
            type: 'rejected',
            error: error?.stack || String(error),
          });
        }
      });
    `,
    { eval: true },
  );

  const parentLease = await binding.acquireAsyncRuntime();
  let parentLeaseReleased = false;
  const slowSource = `export const retirementLoad = [${Array.from(
    { length: 750_000 },
    (_, index) => index,
  ).join(',')}];`;
  const retirementWork = Promise.allSettled(
    Array.from({ length: 4 }, (_, index) =>
      binding.transform(`retirement-${index}.js`, slowSource, undefined),
    ),
  );

  try {
    assert.equal((await waitForWorkerMessage(worker)).type, 'ready');
    await new Promise((resolve) => setImmediate(resolve));
    parentLease.release();
    parentLeaseReleased = true;
    worker.postMessage('acquire');
    assert.equal((await waitForWorkerMessage(worker)).type, 'acquiring');

    // Whether the acquisition resolves before the worker is terminated is a
    // scheduling race and deliberately not asserted; either outcome must
    // leave the main realm working, which is what this case checks below.
    const settled = await waitForWorkerMessageOrDelay(worker, 25);
    assert.ok(
      settled === undefined || settled.type === 'acquired' || settled.type === 'rejected',
      `unexpected worker acquisition outcome: ${JSON.stringify(settled)}`,
    );
  } finally {
    if (!parentLeaseReleased) {
      parentLease.release();
    }
    await worker.terminate();
    await retirementWork;
  }

  const restartedLease = await withTimeout(
    binding.acquireAsyncRuntime(),
    30_000,
    'main realm could not acquire after cancelling the worker environment',
  );
  restartedLease.release();
  await generateAndClose('restart-after-environment-cancellation');
});

await check('operation rejection releases the runtime for a restart', async () => {
  const operationError = new Error('injected scan failure');
  await assert.rejects(
    scan({
      input: 'virtual:scan-failure',
      plugins: [
        {
          name: 'scan-failure',
          resolveId(id) {
            if (id === 'virtual:scan-failure') return `\0${id}`;
          },
          load(id) {
            if (id === '\0virtual:scan-failure') throw operationError;
          },
        },
      ],
    }),
    (error) => containsError(error, operationError),
  );

  await generateAndClose('restart-after-rejection');
});

await check('construction failures release real runtime leases', async () => {
  const copyRoot = mkdtempSync(path.join(packageDir, '.wasi-construction-copy-'));
  const copyDirectory = path.join(copyRoot, 'dist');
  cpSync(distDir, copyDirectory, { recursive: true });

  const constructionError = new Error('injected BindingBundler construction failure');
  const constructionErrorKey = '__rolldownWasiConstructionError';
  globalThis[constructionErrorKey] = constructionError;
  const bindingExportForwarders = Object.keys(binding)
    .filter((name) => /^[$A-Z_a-z][$\w]*$/.test(name))
    .map((name) => `module.exports.${name} = binding.${name};`)
    .join('\n');
  writeFileSync(
    path.join(copyDirectory, 'rolldown-binding.wasi.cjs'),
    `
      const binding = require(${JSON.stringify(bindingPath)});
      ${bindingExportForwarders}
      module.exports.BindingBundler = class {
        constructor() {
          throw globalThis[${JSON.stringify(constructionErrorKey)}];
        }
      };
    `,
  );

  try {
    const [failingRolldown, failingExperimental] = await Promise.all([
      import(pathToFileURL(path.join(copyDirectory, 'index.mjs')).href),
      import(pathToFileURL(path.join(copyDirectory, 'experimental-index.mjs')).href),
    ]);
    await assert.rejects(
      failingRolldown.rolldown({ input: 'virtual:construction-failure' }),
      (error) => containsError(error, constructionError),
    );
    await assert.rejects(
      failingExperimental.scan({ input: 'virtual:scan-construction-failure' }),
      (error) => containsError(error, constructionError),
    );
  } finally {
    delete globalThis[constructionErrorKey];
    rmSync(copyRoot, { force: true, recursive: true });
  }

  await generateAndClose('restart-after-construction-failure');
});

// `dev()` needs a MultiThread executor to finish its initial build, and no
// WebAssembly artifact has one, so the public entry must fail closed through the
// capability contract instead of stalling on a build that can never complete.
// The rejection must also be repeatable and must leave the runtime usable.
await check(
  'dev is rejected by the capability contract and leaves the runtime usable',
  async () => {
    for (const label of ['threaded-wasi-dev-first', 'threaded-wasi-dev-restart']) {
      await assert.rejects(runVirtualDevEngine(label), isDevUnsupported);
    }

    await generateAndClose('restart-after-unsupported-dev');
  },
);

// The original case drove a real dev engine through a cancelled close callback.
// That scenario is unreachable now that dev is rejected, but the copied-package
// machinery still proves something worth proving: the gate lives above the
// binding, so a second package copy cannot reach `BindingDevEngine` and cannot
// strand a runtime lease on its way to the rejection.
await check('a package copy cannot reach the dev binding behind the capability gate', async () => {
  const copyRoot = mkdtempSync(path.join(packageDir, '.wasi-dev-close-copy-'));
  const copyDirectory = path.join(copyRoot, 'dist');
  cpSync(distDir, copyDirectory, { recursive: true });

  const captureKey = '__rolldownWasiDevCloseCapture';
  const capture = {};
  globalThis[captureKey] = capture;
  const bindingExportForwarders = Object.keys(binding)
    .filter((name) => /^[$A-Z_a-z][$\w]*$/.test(name))
    .map((name) => `module.exports.${name} = binding.${name};`)
    .join('\n');
  writeFileSync(
    path.join(copyDirectory, 'rolldown-binding.wasi.cjs'),
    `
      const binding = require(${JSON.stringify(bindingPath)});
      ${bindingExportForwarders}
      module.exports.acquireAsyncRuntime = async function() {
        const runtimeLease = await binding.acquireAsyncRuntime();
        globalThis[${JSON.stringify(captureKey)}].runtimeLease = runtimeLease;
        return runtimeLease;
      };
      module.exports.BindingDevEngine = class {
        constructor(...args) {
          const engine = new binding.BindingDevEngine(...args);
          globalThis[${JSON.stringify(captureKey)}].engine = engine;
          return engine;
        }
      };
    `,
  );

  try {
    const copiedExperimental = await import(
      pathToFileURL(path.join(copyDirectory, 'experimental-index.mjs')).href
    );
    const id = 'virtual:cancelled-callback-close';
    let outputCallbackCalls = 0;
    await assert.rejects(
      copiedExperimental.dev(
        {
          input: id,
          experimental: { devMode: true },
          plugins: [
            {
              name: 'cancelled-callback-close',
              resolveId(source) {
                if (source === id) return `\0${source}`;
              },
              load(source) {
                if (source === `\0${id}`) return 'export const cancelledCallbackClose = true;';
              },
            },
          ],
        },
        {},
        {
          onOutput() {
            outputCallbackCalls += 1;
          },
        },
      ),
      isDevUnsupported,
    );

    assert.equal(outputCallbackCalls, 0, 'the rejected dev entry must not emit output');
    assert.equal(
      capture.engine,
      undefined,
      'the capability gate must reject before constructing BindingDevEngine',
    );
    assert.equal(
      capture.runtimeLease,
      undefined,
      'the rejected dev entry must not strand a runtime lease',
    );
  } finally {
    capture.runtimeLease?.release();
    delete globalThis[captureKey];
    rmSync(copyRoot, { force: true, recursive: true });
  }

  await generateAndClose('restart-after-rejected-copy-dev');
});

await check('a worker realm acquires, uses, and releases its own runtime lease', async () => {
  const worker = new Worker(
    `
      const { parentPort } = require('node:worker_threads');
      (async () => {
        const { rolldown } = await import('rolldown');
        const { getRuntimeCapabilities } = await import('rolldown/experimental');
        const id = 'virtual:worker-runtime';
        const bundle = await rolldown({
          input: id,
          plugins: [{
            name: 'worker-runtime',
            resolveId(source) {
              if (source === id) return '\\0' + source;
            },
            load(source) {
              if (source === '\\0' + id) return 'export const workerRuntime = true;';
            },
          }],
        });
        let result;
        try {
          const output = await bundle.generate();
          result = {
            code: output.output[0].code,
            target: getRuntimeCapabilities().target,
          };
        } finally {
          await bundle.close();
        }
        parentPort.postMessage(result);
      })().catch((error) => {
        parentPort.postMessage({ error: error?.stack || String(error) });
      });
    `,
    { eval: true },
  );

  try {
    const exitPromise = waitForWorkerExit(worker);
    const result = await waitForWorkerMessage(worker);
    assert.equal(result.error, undefined);
    assert.equal(result.target, 'wasi-threads');
    assert.match(result.code, /workerRuntime/);
    assert.equal(await exitPromise, 0);
  } finally {
    await worker.terminate();
  }
});

await check('parallel plugins fail closed without affecting runtime restart', async () => {
  assert.throws(
    () =>
      defineParallelPlugin(
        path.join(import.meta.dirname, 'build-api', 'parallel-close-plugin.mjs'),
      ),
    (error) =>
      error?.code === 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE' &&
      error?.feature === 'parallelPlugins',
  );

  const descriptor = {
    _parallel: {
      fileUrl: pathToFileURL(
        path.join(import.meta.dirname, 'build-api', 'parallel-close-plugin.mjs'),
      ).href,
      options: {},
    },
  };
  let pluginPromiseThenCalls = 0;
  let inputOptionsHookCalls = 0;
  let outputOptionsHookCalls = 0;
  const hangingPlugin = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies preflight before promise assimilation
    then() {
      pluginPromiseThenCalls += 1;
      return new Promise(() => {});
    },
  };

  await assert.rejects(
    withTimeout(
      rolldown({
        input: 'virtual:fabricated-parallel-plugin',
        plugins: [
          hangingPlugin,
          {
            name: 'input-options-side-effect',
            options(options) {
              inputOptionsHookCalls += 1;
              return options;
            },
          },
          descriptor,
        ],
      }),
      5_000,
      'rolldown descriptor preflight awaited a plugin promise',
    ),
    isParallelPluginUnsupported,
  );

  await assert.rejects(
    withTimeout(
      build({
        input: 'virtual:fabricated-parallel-output-plugin',
        plugins: [hangingPlugin],
        output: {
          plugins: [
            {
              name: 'output-options-side-effect',
              outputOptions(options) {
                outputOptionsHookCalls += 1;
                return options;
              },
            },
            descriptor,
          ],
        },
        write: false,
      }),
      5_000,
      'build descriptor preflight awaited a plugin promise',
    ),
    isParallelPluginUnsupported,
  );

  await assert.rejects(
    withTimeout(
      scan(
        {
          input: 'virtual:fabricated-parallel-scan-plugin',
          plugins: [hangingPlugin],
        },
        {
          plugins: [
            {
              name: 'scan-output-options-side-effect',
              outputOptions(options) {
                outputOptionsHookCalls += 1;
                return options;
              },
            },
            descriptor,
          ],
        },
      ),
      5_000,
      'scan descriptor preflight awaited a plugin promise',
    ),
    isParallelPluginUnsupported,
  );

  assert.equal(pluginPromiseThenCalls, 0);
  assert.equal(inputOptionsHookCalls, 0);
  assert.equal(outputOptionsHookCalls, 0);

  await generateAndClose('restart-after-parallel-plugin-rejection');
});

await check('duplicate package copies share one binding-backed lease manager', async () => {
  const copiesRoot = mkdtempSync(path.join(packageDir, '.wasi-lifecycle-copies-'));
  const copyDirectories = [path.join(copiesRoot, 'copy-a'), path.join(copiesRoot, 'copy-b')];
  try {
    for (const copyDirectory of copyDirectories) {
      cpSync(distDir, copyDirectory, { recursive: true });
      const copiedBinding = path.join(copyDirectory, 'rolldown-binding.wasi.cjs');
      rmSync(copiedBinding);
      symlinkSync(bindingPath, copiedBinding);
    }

    const [firstCopy, secondCopy] = await Promise.all(
      copyDirectories.map(
        (copyDirectory) => import(pathToFileURL(path.join(copyDirectory, 'index.mjs')).href),
      ),
    );
    const [first, second] = await Promise.all([
      createVirtualBundle('duplicate-first', firstCopy.rolldown),
      createVirtualBundle('duplicate-second', secondCopy.rolldown),
    ]);
    try {
      await Promise.all([first.generate(), second.generate()]);
      await first.close();
      const output = await second.generate();
      assert.match(output.output[0].code, /duplicate-second/);
    } finally {
      await Promise.allSettled([first.close(), second.close()]);
    }

    const restarted = await createVirtualBundle('duplicate-restart', firstCopy.rolldown);
    try {
      await restarted.generate();
    } finally {
      await restarted.close();
    }
  } finally {
    rmSync(copiesRoot, { force: true, recursive: true });
  }
});

console.log(JSON.stringify({ completed, target: getRuntimeCapabilities().target }));

async function check(name, operation) {
  await withTimeout(Promise.resolve().then(operation), 60_000, `${name} timed out`);
  completed.push(name);
  console.log(`ok - ${name}`);
}

function createVirtualBundle(label, create = rolldown) {
  const id = `virtual:${label}`;
  return create({
    input: id,
    plugins: [
      {
        name: label,
        resolveId(source) {
          if (source === id) return `\0${source}`;
        },
        load(source) {
          if (source === `\0${id}`) return `export const value = ${JSON.stringify(label)};`;
        },
      },
    ],
  });
}

async function generateAndClose(label) {
  const bundle = await createVirtualBundle(label);
  try {
    const output = await bundle.generate();
    assert.match(output.output[0].code, new RegExp(label));
  } finally {
    await bundle.close();
  }
}

async function runVirtualDevEngine(label) {
  const id = `virtual:${label}`;
  let resolveOutput;
  let rejectOutput;
  const outputPromise = new Promise((resolve, reject) => {
    resolveOutput = resolve;
    rejectOutput = reject;
  });
  const engine = await dev(
    {
      input: id,
      experimental: { devMode: true },
      plugins: [
        {
          name: label,
          resolveId(source) {
            if (source === id) return `\0${source}`;
          },
          load(source) {
            if (source === `\0${id}`) {
              return `export const value = ${JSON.stringify(label)};`;
            }
          },
        },
      ],
    },
    {},
    {
      onOutput(output) {
        if (output instanceof Error) {
          rejectOutput(output);
        } else {
          resolveOutput(output);
        }
      },
    },
  );
  try {
    await engine.run();
    const output = await withTimeout(
      outputPromise,
      30_000,
      `dev engine did not emit output for ${label}`,
    );
    assert.match(output.output[0].code, new RegExp(label));
  } finally {
    await engine.close();
  }
}

function isParallelPluginUnsupported(error) {
  return (
    error?.code === 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE' &&
    error?.feature === 'parallelPlugins'
  );
}

function isDevUnsupported(error) {
  return error?.code === 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE' && error?.feature === 'dev';
}

function containsError(error, expected) {
  if (error === expected) return true;
  if (
    error instanceof Error &&
    expected instanceof Error &&
    error.name === expected.name &&
    error.message === expected.message
  ) {
    return true;
  }
  const nestedErrors =
    typeof error === 'object' && error !== null && Array.isArray(error.errors) ? error.errors : [];
  return nestedErrors.some((entry) => containsError(entry, expected));
}

function waitForWorkerMessageOrDelay(worker, milliseconds) {
  return new Promise((resolve, reject) => {
    const onMessage = (message) => {
      clearTimeout(timer);
      worker.off('error', onError);
      resolve(message);
    };
    const onError = (error) => {
      clearTimeout(timer);
      worker.off('message', onMessage);
      reject(error);
    };
    const timer = setTimeout(() => {
      worker.off('message', onMessage);
      worker.off('error', onError);
      resolve(undefined);
    }, milliseconds);
    worker.once('message', onMessage);
    worker.once('error', onError);
  });
}

function waitForWorkerMessage(worker) {
  return withTimeout(
    new Promise((resolve, reject) => {
      worker.once('message', resolve);
      worker.once('error', reject);
    }),
    30_000,
    'worker realm did not report its result',
  );
}

function waitForWorkerExit(worker) {
  return withTimeout(
    new Promise((resolve, reject) => {
      worker.once('exit', resolve);
      worker.once('error', reject);
    }),
    30_000,
    'worker realm did not exit after releasing its runtime lease',
  );
}

function withTimeout(promise, milliseconds, message) {
  let timer;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), milliseconds);
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}
