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
const { defineParallelPlugin, dev, getRuntimeSupport, scan } = experimentalApi;

const require = createRequire(import.meta.url);
const packageDir = path.dirname(require.resolve('rolldown/package.json'));
const distDir = path.join(packageDir, 'dist');
const bindingPath = path.join(distDir, 'rolldown-binding.wasi.cjs');
const binding = require(bindingPath);
const completed = [];

// The raw loader's report; the public API exposes only `getRuntimeSupport()`.
const runtimeCapabilities = binding.getRuntimeCapabilities();

assert.equal(
  runtimeCapabilities.target,
  'wasi-threads',
  'the WASI lifecycle suite must run against the threaded-WASI artifact',
);
// This suite pins the lane's flavor. The threaded artifact defaults to MultiThread;
// `ROLLDOWN_RUNTIME=single` selects CurrentThread. CI runs the suite both ways. See
// `resolve_runtime_config_for` in crates/rolldown_binding/src/async_runtime.rs.
const laneIsSingle = ['single', 'single-thread', 'current', 'current-thread'].includes(
  process.env.ROLLDOWN_RUNTIME ?? '',
);
const expectedFlavor = laneIsSingle ? 'CurrentThread' : 'MultiThread';
assert.equal(
  runtimeCapabilities.flavor,
  expectedFlavor,
  laneIsSingle
    ? 'ROLLDOWN_RUNTIME=single selects CurrentThread on the threaded-WASI artifact'
    : 'the threaded-WASI artifact defaults to the shared scheduler MultiThread flavor',
);
assert.equal(
  runtimeCapabilities.threads,
  !laneIsSingle,
  laneIsSingle
    ? 'CurrentThread schedules on the calling lane only'
    : 'MultiThread runs the scheduler on its own worker threads',
);
assert.deepEqual(getRuntimeSupport(), {
  dev: !laneIsSingle,
  watch: false,
  parallelPlugins: false,
  threadlessWasi: false,
  workerd: false,
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
        const { getRuntimeSupport } = await import('rolldown/experimental');
        console.log(JSON.stringify(getRuntimeSupport()));
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
  assert.deepEqual(JSON.parse(probe.stdout.trim().split('\n').at(-1)), getRuntimeSupport());
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

await check('watch throws before setup', () => {
  let optionsHookCalls = 0;
  assert.throws(
    () =>
      watch({
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
      }),
    (error) =>
      error?.code === 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE' && error?.feature === 'watch',
  );
  assert.equal(optionsHookCalls, 0);
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

await check('construction failures leave the shared runtime usable', async () => {
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

// `dev()` needs MultiThread: under CurrentThread it must fail closed through the
// capability contract instead of stalling on a build that can never complete.
if (laneIsSingle) {
  await check(
    'dev is rejected by the capability contract and leaves the runtime usable',
    async () => {
      for (const label of ['threaded-wasi-dev-first', 'threaded-wasi-dev-restart']) {
        await assert.rejects(runVirtualDevEngine(label), isDevUnsupported);
      }

      await generateAndClose('restart-after-unsupported-dev');
    },
  );
} else {
  await check('dev builds, closes and leaves the runtime usable', async () => {
    for (const label of ['threaded-wasi-dev-first', 'threaded-wasi-dev-restart']) {
      await runVirtualDevEngine(label);
    }

    await generateAndClose('restart-after-dev');
  });
}

await check('a worker realm builds and closes in its own environment', async () => {
  const worker = new Worker(
    `
      const { parentPort } = require('node:worker_threads');
      (async () => {
        const { rolldown } = await import('rolldown');
        const { getRuntimeSupport } = await import('rolldown/experimental');
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
            support: getRuntimeSupport(),
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
    assert.deepEqual(result.support, getRuntimeSupport());
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
  let inputOptionsHookCalls = 0;

  await assert.rejects(
    withTimeout(
      rolldown({
        input: 'virtual:fabricated-parallel-plugin',
        plugins: [
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
      'rolldown descriptor check did not reject',
    ),
    isParallelPluginUnsupported,
  );

  await assert.rejects(
    withTimeout(
      build({
        input: 'virtual:fabricated-parallel-output-plugin',
        output: {
          plugins: [descriptor],
        },
        write: false,
      }),
      5_000,
      'build output descriptor check did not reject',
    ),
    isParallelPluginUnsupported,
  );

  await assert.rejects(
    withTimeout(
      scan(
        {
          input: 'virtual:fabricated-parallel-scan-plugin',
        },
        {
          plugins: [descriptor],
        },
      ),
      5_000,
      'scan output descriptor check did not reject',
    ),
    isParallelPluginUnsupported,
  );

  assert.equal(inputOptionsHookCalls, 0);

  await generateAndClose('restart-after-parallel-plugin-rejection');
});

await check('duplicate package copies share one binding', async () => {
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

console.log(JSON.stringify({ completed, target: runtimeCapabilities.target }));

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
    'worker realm did not exit after closing its bundle',
  );
}

function withTimeout(promise, milliseconds, message) {
  let timer;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), milliseconds);
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}
