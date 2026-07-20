import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import nodePath from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { Worker } from 'node:worker_threads';
// Ensures the timer host is registered before `timers` is asserted: importing
// the rolldown entry runs `setup.ts` -> `timer-host.ts` -> `registerTimerHost`
// (the same side effect every public binding-loading entry now carries).
import { rolldown, watch } from 'rolldown';
import {
  dev,
  defineParallelPlugin,
  getAsyncRuntimeConfig,
  getRuntimeCapabilities,
  getRuntimeSupport,
  viteDynamicImportVarsPlugin,
} from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

// This spec runs against whatever binding the worktree built (native in
// either shared-runtime flavor, or a WASI artifact) and asserts the
// capability report is internally coherent and matches the artifact. The
// per-lane correctness of the individual flags is additionally pinned by the
// suites themselves: every `isWasiTest` / `isSingleThread` skip predicate in
// tests/src/runtime-flavor.ts is now derived from this report, so a lying
// capability shifts the lane's pass/skip counts immediately.

const testsDir = fileURLToPath(new URL('.', import.meta.url));

// Run `script` (an ESM module body) in a FRESH node process that resolves
// packages from this tests package; returns the last stdout line parsed as
// JSON (stderr, e.g. node:wasi's ExperimentalWarning, is ignored).
function inFreshProcess(script: string, env: Record<string, string | undefined> = {}): any {
  const stdout = execFileSync(process.execPath, ['--input-type=module', '-e', script], {
    cwd: testsDir,
    env: { ...process.env, ...env },
    encoding: 'utf8',
    timeout: 30_000,
  });
  const lines = stdout.trim().split('\n');
  return JSON.parse(lines[lines.length - 1]);
}

describe('getRuntimeCapabilities', () => {
  const caps = getRuntimeCapabilities();
  // Every shared-runtime artifact has a timer facility once a public entry
  // has loaded: MultiThread owns a timer heap, CurrentThread delegates to the
  // registered host driver.
  const timersExpected = true;

  test('reports a coherent capability set for the loaded artifact', () => {
    expect(caps.backend).toBe('shared');
    expect(caps.asyncRuntimeBuild).toBe(true);

    expect(['native', 'wasi', 'wasi-threads']).toContain(caps.target);
    expect(caps.wasi).toBe(caps.target !== 'native');

    expect(['CurrentThread', 'MultiThread']).toContain(caps.flavor);
    expect(caps.threads).toBe(caps.flavor === 'MultiThread');
    // Worker threads keep native tasks progressing, but a foreign block_on
    // entered on Node's main thread still parks the JS event loop.
    expect(caps.blockOnJsThreadSafe).toBe(false);

    // One pipeline: the capability flavor and the config reporter's flavor
    // come from the same resolved snapshot / runtime controller.
    expect(caps.flavor).toBe(getAsyncRuntimeConfig().flavor);

    // Static per artifact: watch works on both native flavors and on no wasm
    // artifact, independent of timer-host registration state.
    expect(caps.watchSupported).toBe(!caps.wasi);
    expect(caps.devSupported).toBe(caps.threads);

    // Every shared-runtime WebAssembly artifact -- wasi and wasi-threads
    // alike -- schedules on the calling thread only.
    if (caps.wasi) {
      expect(caps.flavor).toBe('CurrentThread');
      expect(caps.threads).toBe(false);
      expect(getAsyncRuntimeConfig()).toMatchObject({
        flavor: 'CurrentThread',
        maxBlockingTasks: 1,
        workerThreads: 1,
      });
    }
  });

  test('reports complete public workflow support', () => {
    const support = getRuntimeSupport();
    expect(support).toEqual({
      dev: caps.devSupported,
      watch: caps.watchSupported,
      dynamicImportVarsResolver: true,
      importGlobResolver: true,
      parallelPlugins: !caps.wasi,
      pluginErrorMetadata: true,
      symlinks: !caps.wasi,
      threadlessWasi: caps.target === 'wasi' && !caps.threads,
      workerd: false,
    });
    expect(Object.keys(support)).toContain('workerd');
    expect(Object.getOwnPropertyDescriptor(support, 'workerd')).toMatchObject({
      enumerable: true,
      value: false,
    });
  });

  test('preserves structured plugin error metadata', async () => {
    const cause = Object.assign(new RangeError('nested plugin cause'), {
      nestedMarker: 17,
    });
    const original = Object.assign(new TypeError('plugin metadata failure'), {
      cause,
      code: 'USER_PLUGIN_CODE',
      customMarker: 'retained',
    });
    const bundle = await rolldown({
      input: 'entry',
      plugins: [
        {
          name: 'runtime-metadata-probe',
          resolveId(id) {
            if (id === 'entry') return '\0entry';
          },
          load(id) {
            if (id === '\0entry') return 'export default 1';
          },
          transform(_code, id) {
            if (id === '\0entry') throw original;
          },
        },
      ],
    });

    try {
      const failure = await bundle.generate().catch((error: unknown) => error);
      const [pluginError] = (failure as { errors?: unknown[] }).errors ?? [];
      expect(pluginError).toBe(original);
      expect(pluginError).toMatchObject({
        code: 'PLUGIN_ERROR',
        pluginCode: 'USER_PLUGIN_CODE',
        plugin: 'runtime-metadata-probe',
        hook: 'transform',
        id: '\0entry',
        customMarker: 'retained',
      });
      expect(original.stack).toContain('plugin metadata failure');
      expect(original.cause).toBe(cause);
      expect(original.cause).toMatchObject({
        name: 'RangeError',
        message: 'nested plugin cause',
        nestedMarker: 17,
      });
    } finally {
      await bundle.close();
    }
  });

  test('JS-backed dynamic import resolvers build through the async callback bridge', async () => {
    const fixtureDir = nodePath.join(
      import.meta.dirname,
      'fixtures/builtin-plugin/dynamic-import-vars/vite',
    );
    let resolverCalls = 0;
    const bundle = await rolldown({
      input: nodePath.join(fixtureDir, 'main.js'),
      plugins: [
        viteDynamicImportVarsPlugin({
          async resolver(id) {
            resolverCalls += 1;
            return id
              .replace('@', nodePath.join(fixtureDir, 'mods'))
              .replace('#', nodePath.resolve(fixtureDir, '../../'));
          },
        }),
      ],
    });

    try {
      await expect(bundle.generate()).resolves.toBeDefined();
      expect(resolverCalls).toBeGreaterThan(0);
    } finally {
      await bundle.close();
    }
  });

  test.runIf(caps.flavor === 'CurrentThread')('dev fails before entering the binding', async () => {
    await expect(dev({ input: 'entry.js' })).rejects.toMatchObject({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'dev',
    });
  });

  test.runIf(caps.wasi)('watch reports a normal setup failure instead of stalling', async () => {
    const watcher = watch({ input: 'entry.js' });
    const events: string[] = [];
    let reportedError: Error | undefined;
    const ended = new Promise<void>((resolve) => {
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
    expect(events).toEqual(['ERROR', 'END']);
    expect(reportedError).toMatchObject({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'watch',
    });
  });

  test.runIf(caps.wasi)('parallel plugins fail before spawning workers on WASI', () => {
    expect(() => defineParallelPlugin('file:///parallel-plugin.mjs')).toThrowError(
      expect.objectContaining({
        code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
        feature: 'parallelPlugins',
      }),
    );
  });
  test('a timer facility is available once any public entry has loaded', () => {
    // The shared MultiThread flavor owns a timer heap; shared CurrentThread
    // delegates to the host driver registered by the entry's timer-host side
    // effect.
    expect(caps.timers).toBe(timersExpected);
  });

  test('the report is a stable snapshot, not an env re-read', () => {
    const capsBefore = getRuntimeCapabilities();
    const configBefore = getAsyncRuntimeConfig();
    const mutated = { ...process.env };
    process.env.ROLLDOWN_RUNTIME = caps.threads ? 'single' : 'multi';
    process.env.ROLLDOWN_WORKER_THREADS = '99';
    try {
      expect(getRuntimeCapabilities()).toEqual(capsBefore);
      expect(getAsyncRuntimeConfig()).toEqual(configBefore);
    } finally {
      process.env.ROLLDOWN_RUNTIME = mutated.ROLLDOWN_RUNTIME;
      process.env.ROLLDOWN_WORKER_THREADS = mutated.ROLLDOWN_WORKER_THREADS;
      if (mutated.ROLLDOWN_RUNTIME === undefined) {
        delete process.env.ROLLDOWN_RUNTIME;
      }
      if (mutated.ROLLDOWN_WORKER_THREADS === undefined) {
        delete process.env.ROLLDOWN_WORKER_THREADS;
      }
    }
  });

  // Import-order invariant: the capability contract must not depend on which
  // public entry loads first. A fresh process whose ONLY import is
  // `rolldown/experimental` must still see working timers (the entry carries
  // the timer-host side effect itself) and the artifact-static watch flag.
  // Without that side effect, a CurrentThread artifact reports timers:false
  // and watchSupported:false despite supporting both.
  test('capabilities do not depend on import order (experimental-only import)', () => {
    const fresh = inFreshProcess(`
      const { getRuntimeCapabilities } = await import('rolldown/experimental');
      console.log(JSON.stringify(getRuntimeCapabilities()));
    `);
    expect(fresh.timers).toBe(timersExpected);
    expect(fresh.watchSupported).toBe(!caps.wasi);
    // Same artifact, same lane env => the flavor this process sees.
    expect(fresh.flavor).toBe(caps.flavor);
  });

  // Load-time snapshot invariant: every artifact resolves the runtime config
  // eagerly in lib.rs `init`, so an env
  // mutation between import and the FIRST query is invisible -- the report
  // must equal a control process that queried immediately. Threaded-WASI
  // note: node:wasi additionally snapshots the WASI env at loader load
  // (uvwasi), which masked the lazy-resolution hole on this particular host;
  // the eager init makes load-time pinning a property of rolldown itself
  // rather than of the host's WASI shim.
  test('first query reflects load-time env even if mutated after import', () => {
    const spawnEnv = {
      ROLLDOWN_WORKER_THREADS: '7',
      NAPI_RS_ASYNC_WORK_POOL_SIZE: '6',
    };
    const report = `
      const { getAsyncRuntimeConfig, getRuntimeCapabilities } = await import('rolldown/experimental');
      MUTATE;
      console.log(JSON.stringify({ config: getAsyncRuntimeConfig(), caps: getRuntimeCapabilities() }));
    `;
    const mutateAll = `for (const key of ['ROLLDOWN_RUNTIME', 'ROLLDOWN_WORKER_THREADS', 'ROLLDOWN_MAX_BLOCKING_THREADS', 'ROLLDOWN_PARK_DEADLINE_MS', 'NAPI_RS_ASYNC_WORK_POOL_SIZE', 'UV_THREADPOOL_SIZE']) process.env[key] = '9'`;
    const mutated = inFreshProcess(report.replace('MUTATE', mutateAll), spawnEnv);
    const control = inFreshProcess(report.replace('MUTATE', ''), spawnEnv);
    expect(mutated).toEqual(control);
  });

  test.runIf(caps.target === 'native')(
    'oversized thread environment values are bounded before addon import',
    () => {
      const report = inFreshProcess(
        `
          const { getAsyncRuntimeConfig } = await import('rolldown/experimental');
          console.log(JSON.stringify(getAsyncRuntimeConfig()));
        `,
        {
          ROLLDOWN_MAX_BLOCKING_THREADS: '1000000',
          ROLLDOWN_RUNTIME: 'multi',
          ROLLDOWN_WORKER_THREADS: '1000000',
        },
      );
      expect(report.workerThreads).toBe(256);
      expect(report.maxBlockingTasks).toBe(255);
    },
  );

  // Worker-environment invariant: the contract must hold inside Node
  // worker_threads too. Timer-host registration carries NO isMainThread guard
  // because the parallel-plugin machinery loads the binding in workers. A
  // fresh process whose FIRST binding import happens inside a worker must see
  // working timers there. The native driver registry takes one registration
  // per importing env and races every timer across all live hosts, so the
  // worker's registration joins rather than clobbers a later main-thread
  // import. Without worker-side registration, a CurrentThread worker reports
  // timers:false and a CT sleep_until there panics driverless even though
  // watchSupported is statically true.
  test('capabilities hold when a worker thread imports the binding first', () => {
    const result = inFreshProcess(`
      import { Worker } from 'node:worker_threads';
      const workerCaps = await new Promise((resolve, reject) => {
        const worker = new Worker(
          "(async () => { const { getRuntimeCapabilities } = await import('rolldown/experimental'); const { parentPort } = await import('node:worker_threads'); parentPort.postMessage(getRuntimeCapabilities()); })().catch((error) => { setTimeout(() => { throw error; }); })",
          { eval: true },
        );
        worker.once('message', resolve);
        worker.once('error', reject);
      });
      // The worker registered first; the main thread must not be driverless
      // either (it registers its own per-env driver alongside the worker's).
      const { getRuntimeCapabilities } = await import('rolldown/experimental');
      console.log(JSON.stringify({ workerCaps, mainCaps: getRuntimeCapabilities() }));
    `);
    expect(result.workerCaps.timers).toBe(timersExpected);
    expect(result.workerCaps.watchSupported).toBe(!caps.wasi);
    expect(result.workerCaps.flavor).toBe(caps.flavor);
    expect(result.mainCaps.timers).toBe(timersExpected);
  });

  // Driver-lifetime invariant: a worker that imports the binding FIRST and
  // then EXITS must not leave timer duty to its dead driver. The weak
  // threadsafe function behind the timer host does not keep the worker's
  // event loop alive, so the worker exits naturally and its env teardown
  // kills the callback. A first-registration-wins slot would let that dead
  // driver shadow the live main-thread host; a later main-thread watch
  // debounce (a REAL CurrentThread sleep -- buildDelay > 0 keeps it off the
  // already-elapsed fast path) would then busy-fail against the dead callback.
  // The registry must evict the dead registrant, re-arm on the main thread's
  // live driver, and keep stderr clean.
  test.skipIf(!caps.watchSupported)(
    'watch debounce timers survive a worker-first registrant that exited',
    { retry: 3, timeout: 60_000 },
    () => {
      const child = spawnSync(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `
      import { Worker } from 'node:worker_threads';
      import fs from 'node:fs';
      import os from 'node:os';
      import path from 'node:path';

      // Step 1: a worker imports the binding FIRST (registering a timer host
      // owned by the worker's env), then exits.
      const worker = new Worker(
        "(async () => { await import('rolldown/experimental'); const { parentPort } = await import('node:worker_threads'); parentPort.postMessage('ready'); })().catch((error) => { setTimeout(() => { throw error; }); })",
        { eval: true },
      );
      await new Promise((resolve, reject) => {
        worker.once('message', resolve);
        worker.once('error', reject);
      });
      const naturalExit = await Promise.race([
        new Promise((resolve) => worker.once('exit', () => resolve(true))),
        new Promise((resolve) => setTimeout(() => resolve(false), 10_000)),
      ]);
      if (!naturalExit) {
        // Fallback: terminate tears the env down just the same.
        await worker.terminate();
      }

      // Step 2: the main thread imports rolldown and runs a REAL watch whose
      // debounce must go through a CurrentThread host timer.
      const { watch } = await import('rolldown');
      const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rd-worker-exit-timer-'));
      const input = path.join(dir, 'main.js');
      fs.writeFileSync(input, 'export const a = 1;');
      const watcher = watch({
        input,
        cwd: dir,
        output: { dir: path.join(dir, 'dist') },
        watch: { buildDelay: 150, watcher: { usePolling: true, pollInterval: 50 } },
      });

      let endCount = 0;
      let resolveSecondEnd;
      const secondEnd = new Promise((resolve) => { resolveSecondEnd = resolve; });
      watcher.on('event', (event) => {
        if (event.code === 'END') {
          endCount += 1;
          if (endCount >= 2) resolveSecondEnd(true);
        }
      });
      await new Promise((resolve) => {
        const check = setInterval(() => {
          if (endCount >= 1) { clearInterval(check); resolve(); }
        }, 50);
      });

      // Edit across a whole-second mtime boundary (poll watcher granularity).
      await new Promise((resolve) => setTimeout(resolve, 1100));
      fs.writeFileSync(input, 'export const a = 2;');
      const rebuilt = await Promise.race([
        secondEnd,
        new Promise((resolve) => setTimeout(() => resolve(false), 20_000)),
      ]);
      console.log(JSON.stringify({ rebuilt, endCount, naturalExit }));
      process.exit(rebuilt ? 0 : 7);
          `,
        ],
        { cwd: testsDir, env: { ...process.env }, encoding: 'utf8', timeout: 55_000 },
      );
      const lines = child.stdout.trim().split('\n');
      const result = JSON.parse(lines[lines.length - 1]);
      expect(result.rebuilt).toBe(true);
      expect(result.endCount).toBeGreaterThanOrEqual(2);
      // A busy retry loop against the dead worker-owned callback emits this
      // line repeatedly. Correct eviction and re-arming never touch the dead
      // driver's relay.
      expect(child.stderr).not.toContain('host timer callback failed');
      expect(child.status).toBe(0);
    },
  );

  // Worker-entry invariant: the REAL `#parallel-plugin-worker` entry loads the
  // binding, so it must import './timer-host' first and carry its own timer-host
  // registration. On native the
  // process-global registry can mask a missing registration (main's driver
  // serves); on the wasm artifacts the registry is per-instance and the
  // worker would be genuinely driverless. Bundling may place `require_binding`
  // and timer-host's top-level registration in one shared chunk, so this test
  // also guards the contract across future chunk splits.
  const rolldownPkgDir = nodePath.dirname(
    createRequire(import.meta.url).resolve('rolldown/package.json'),
  );
  const parallelWorkerEntry = nodePath.join(rolldownPkgDir, 'dist', 'parallel-plugin-worker.mjs');
  const wasiNodeBinding = nodePath.join(rolldownPkgDir, 'dist', 'rolldown-binding.wasi.cjs');
  const wasiArtifact = nodePath.join(rolldownPkgDir, 'dist', 'rolldown-binding.wasm32-wasi.wasm');

  test.runIf(caps.target === 'wasi-threads')(
    'threaded-WASI file workers discard inherited string-input execArgv',
    { timeout: 30_000 },
    () => {
      const result = inFreshProcess(`
        await import(${JSON.stringify(pathToFileURL(wasiNodeBinding).href)});
        await new Promise((resolve) => setTimeout(resolve, 250));
        console.log(JSON.stringify({ loaded: true }));
      `);
      expect(result).toEqual({ loaded: true });
    },
  );

  test.runIf(caps.target === 'wasi-threads')('threaded-WASI artifact is a reactor', () => {
    const module = new WebAssembly.Module(new Uint8Array(readFileSync(wasiArtifact)));
    const exports = WebAssembly.Module.exports(module);
    expect(exports).toContainEqual({ name: '_initialize', kind: 'function' });
    expect(exports.some(({ name }) => name === '_start')).toBe(false);
  });

  test(
    'the parallel-plugin worker entry carries the timer-host registration',
    { timeout: 30_000 },
    async () => {
      expect(existsSync(parallelWorkerEntry)).toBe(true);
      // STRUCTURAL: the entry's static import graph (the entry plus its
      // relative chunks -- including BARE side-effect imports, which is
      // exactly the form the timer-host import takes on the wasi dist) must
      // contain the registration call. Top-level chunk code executes on
      // import, so presence in the graph IS registration in the worker's
      // env. The paired, receiver-bound host bridge must include both timeout
      // creation and cancellation; resolving the relay after clearTimeout lets
      // Rust retire the detached schedule task immediately.
      const entryText = readFileSync(parallelWorkerEntry, 'utf8');
      let graphText = entryText;
      for (const match of entryText.matchAll(/(?:from\s+|import\s+)["'](\.\/[^"']+)["']/g)) {
        const chunkPath = nodePath.join(nodePath.dirname(parallelWorkerEntry), match[1]);
        if (existsSync(chunkPath)) {
          graphText += readFileSync(chunkPath, 'utf8');
        }
      }
      expect(graphText).toContain('Reflect.get(globalThis, "setTimeout"');
      expect(graphText).toContain('Reflect.get(globalThis, "clearTimeout"');
      expect(graphText).toContain('Reflect.apply(timer.clearTimeoutHost');
      expect(graphText).toContain('getCurrentThreadTaskHostContractVersion');
      expect(graphText).toContain('registerCurrentThreadTaskHost');
      expect(graphText).toContain('registerTimerHost');
      expect(graphText).not.toContain('driveCurrentThreadRuntimeTasks');
      expect(graphText).not.toContain('cancelCurrentThreadRuntimeTaskDispatch');
      expect(graphText).toContain('timer.resolve()');

      // BEHAVIORAL: the real entry must actually run as a worker under this
      // lane's flavor. The base bootstrap protocol spawns the entry file
      // directly with `workerData`; an empty plugin set means
      // `registerPlugins(0, [])` no-ops on an unknown registry id and the
      // entry posts `{ type: 'success' }` on `parentPort`.
      const worker = new Worker(parallelWorkerEntry, {
        workerData: {
          registryId: 0,
          pluginInfos: [],
          threadNumber: 0,
          watchMode: false,
        },
      });
      try {
        const outcome: any = await Promise.race([
          new Promise((resolve, reject) => {
            worker.once('message', resolve);
            worker.once('error', reject);
          }),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error('parallel-plugin worker never reported')), 20_000),
          ),
        ]);
        expect(outcome).toEqual({ type: 'success' });
      } finally {
        await worker.terminate();
      }
    },
  );

  // Relay-eviction invariant: eviction must be decided by the unforgeable
  // Closing status or the liveness probe -- NEVER by error
  // message. A rejected JS promise coerces to GenericFailure carrying the
  // JS-controlled rejection string, so a LIVE callback rejecting with
  // Error('oneshot canceled') (colliding with napi's env-died-mid-promise
  // message) must not be misclassified as env death and evicted immediately.
  // It consumes one strike from the 3-strike budget, stays registered, and
  // retries the debounce. Only meaningful where host timers serve watch: the
  // shared CurrentThread flavor on a watch-capable artifact.
  test.skipIf(caps.flavor !== 'CurrentThread' || !caps.watchSupported)(
    'a live timer host rejecting with a colliding message takes the strike path',
    { retry: 3, timeout: 60_000 },
    () => {
      const child = spawnSync(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `
      import fs from 'node:fs';
      import os from 'node:os';
      import path from 'node:path';
      import { createRequire } from 'node:module';
      import { pathToFileURL } from 'node:url';

      const { watch } = await import('rolldown');

      // registerTimerHost is not re-exported publicly; recover the binding's
      // module.exports through the dist shared chunks' require_binding
      // factory (a plain zero-arity function; the chunks are already loaded
      // via the 'rolldown' import above, so this adds no side effects).
      const pkgDir = path.dirname(createRequire(import.meta.url).resolve('rolldown/package.json'));
      const sharedDir = path.join(pkgDir, 'dist', 'shared');
      let binding;
      for (const name of fs.readdirSync(sharedDir)) {
        if (!name.endsWith('.mjs')) continue;
        const chunk = await import(pathToFileURL(path.join(sharedDir, name)));
        for (const value of Object.values(chunk)) {
          if (typeof value !== 'function' || value.length !== 0) continue;
          try {
            const candidate = value();
            if (candidate && typeof candidate.then === 'function') {
              candidate.catch(() => {});
              continue;
            }
            if (candidate && typeof candidate.registerTimerHost === 'function') {
              binding = candidate;
              break;
            }
          } catch {}
        }
        if (binding) break;
      }
      if (!binding) {
        console.log(JSON.stringify({ error: 'binding factory not found' }));
        process.exit(2);
      }

      // A LIVE additional host: rejects its FIRST arm with the colliding
      // message, then behaves normally. Every live registration receives the
      // debounce, so this host must remain present for the retry.
      let calls = 0;
      const active = new Map();
      const registration = binding.reserveCurrentThreadHostRegistration();
      binding.registerTimerHost(
        registration.high,
        registration.low,
        (id, ms) => {
          calls += 1;
          if (calls === 1) return Promise.reject(new Error('oneshot canceled'));
          return new Promise((resolve) => {
            const handle = setTimeout(() => {
              active.delete(id);
              resolve();
            }, ms);
            active.set(id, { handle, resolve });
          });
        },
        (id) => {
          const timer = active.get(id);
          if (!timer) return;
          active.delete(id);
          clearTimeout(timer.handle);
          timer.resolve();
        },
      );

      const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rd-collision-'));
      const input = path.join(dir, 'main.js');
      fs.writeFileSync(input, 'export const a = 1;');
      const watcher = watch({
        input,
        cwd: dir,
        output: { dir: path.join(dir, 'dist') },
        watch: { buildDelay: 150, watcher: { usePolling: true, pollInterval: 50 } },
      });

      let endCount = 0;
      let resolveSecondEnd;
      const secondEnd = new Promise((resolve) => { resolveSecondEnd = resolve; });
      watcher.on('event', (event) => {
        if (event.code === 'END') {
          endCount += 1;
          if (endCount >= 2) resolveSecondEnd(true);
        }
      });
      await new Promise((resolve) => {
        const check = setInterval(() => {
          if (endCount >= 1) { clearInterval(check); resolve(); }
        }, 50);
      });
      await new Promise((resolve) => setTimeout(resolve, 1100));
      fs.writeFileSync(input, 'export const a = 2;');
      const rebuilt = await Promise.race([
        secondEnd,
        new Promise((resolve) => setTimeout(() => resolve(false), 15_000)),
      ]);
      console.log(JSON.stringify({ rebuilt, endCount, calls }));
      process.exit(rebuilt ? 0 : 7);
          `,
        ],
        { cwd: testsDir, env: { ...process.env }, encoding: 'utf8', timeout: 55_000 },
      );
      const lines = child.stdout.trim().split('\n');
      const result = JSON.parse(lines[lines.length - 1]);
      expect(result.rebuilt).toBe(true);
      // The live host must survive its strike and be re-armed.
      expect(result.calls).toBeGreaterThanOrEqual(2);
      expect(child.stderr).toContain('before eviction');
      expect(child.stderr).not.toContain('host gone, evicting');
      expect(child.status).toBe(0);
    },
  );
});
