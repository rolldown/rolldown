import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { Worker } from 'node:worker_threads';

const [rolldownApi, experimentalApi] = await withTimeout(
  Promise.all([import('rolldown'), import('rolldown/experimental')]),
  60_000,
  'the threaded-WASI package did not finish loading',
);
const { build, rolldown, watch } = rolldownApi;
const { defineParallelPlugin, dev, getRuntimeCapabilities, scan } = experimentalApi;

const require = createRequire(import.meta.url);
const packageDir = path.dirname(require.resolve('rolldown/package.json'));
const distDir = path.join(packageDir, 'dist');
const bindingPath = path.join(distDir, 'rolldown-binding.wasi.cjs');
const binding = require(bindingPath);
const completed = [];

assert.equal(
  getRuntimeCapabilities().target,
  'wasi-threads',
  'the WASI lifecycle suite must run against the threaded-WASI artifact',
);
assert.equal(
  getRuntimeCapabilities().devSupported,
  true,
  'threaded WASI must preserve MultiThread dev support',
);
assert.equal(
  typeof binding.acquireAsyncRuntime,
  'function',
  'the generated threaded-WASI binding must export acquireAsyncRuntime',
);

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

await check('immediate token reacquisition waits for Tokio retirement', async () => {
  for (let iteration = 0; iteration < 24; iteration += 1) {
    const lease = await binding.acquireAsyncRuntime();
    lease.release();
  }

  await generateAndClose('restart-after-retirement-stress');
});

await check(
  'environment teardown cancels a runtime acquisition blocked by retirement',
  async () => {
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

      const earlyResult = await waitForWorkerMessageOrDelay(worker, 25);
      assert.equal(
        earlyResult,
        undefined,
        `worker acquisition did not remain pending behind retirement: ${JSON.stringify(earlyResult)}`,
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
  },
);

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

await check('dev engine runs, closes, and restarts on threaded WASI', async () => {
  await runVirtualDevEngine('threaded-wasi-dev-first');
  await runVirtualDevEngine('threaded-wasi-dev-restart');
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
              if (source === id) return '\\\\0' + source;
            },
            load(source) {
              if (source === '\\\\0' + id) return 'export const workerRuntime = true;';
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
