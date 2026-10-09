// Pool worker preload in the threaded WASI Node loader (`__reconcileWasiThreadPool`,
// emitted by @napi-rs/cli): right after the binding loads, it fills emnapi's idle
// Worker pool with `napi_wasm_runtime_pool_workers` Workers.
// See internal-docs/async-runtime/implementation.md ("Pool worker preload").
//
// Each case is a child process that wraps `node:worker_threads` Worker before it
// imports rolldown and counts the pool Workers (`wasi-worker.mjs`) the loader creates:
//   default       MultiThread (default count): N = workerThreads Workers right after
//                 the import, none more for the first builds (a JS plugin runs on
//                 every module, so the pool threads call back into JS)
//   workers-4     the same with ROLLDOWN_WORKER_THREADS=4: 4 preloaded
//   single        ROLLDOWN_RUNTIME=single (CurrentThread): none preloaded (the build
//                 may still spawn a thread of its own, which is not a runtime worker)
//   import-only   imports and returns: the idle preloaded Workers are unref'd, so the
//                 process must exit on its own, code 0, within IMPORT_ONLY_EXIT_MS
//   load-failure  the N preloaded Workers fail to instantiate the wasm: each raises
//                 the loader's crash flag and exits; emnapi puts a fresh, unloaded
//                 Worker in each one's pool slot, so N more exist before the builds,
//                 the builds take those (none more) and succeed; the public
//                 disposer then rejects as latched, with the load failure as cause
//   in-flight     holds a build in a `load` hook, queues transform() calls (napi_async_work)
//                 and calls the public disposer with no await in between: each transform
//                 must end fulfilled or with AbortError (stranded work never settles and
//                 keeps the process alive)
// After in-flight's work, the public disposer must resolve, every pool Worker must
// exit and the loader crash flag must stay 0: terminating idle Workers is not a crash.
// Every case fails on an uncaught exception or unhandled rejection, and a child
// still alive after CASE_TIMEOUT_MS is killed and counts as a hang.
// On the threadless WASI build only `in-flight` runs (no pool Workers, no crash flag).
// Skips (exit 0) on any other artifact, or when MultiThread does not work.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, writeSync } from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import workerThreads from 'node:worker_threads';

const CHILD_FLAG = '--run-pool-worker-preload-case';
const MODULES = 100;
const CONCURRENCY = 4;
const RUNS = 2;
const CASE_TIMEOUT_MS = 30_000;
// After the import-only child prints its counts, it must be gone within this.
const IMPORT_ONLY_EXIT_MS = 5_000;
const LOAD_FAILURE_EXIT_TIMEOUT_MS = 10_000;
const WAIT_TIMEOUT_MS = 10_000;
// The threaded artifact's default MultiThread worker count (`resolve_runtime_config_for`).
const DEFAULT_WORKER_THREADS = 2;
const POOL_WORKER_FILE = 'wasi-worker.mjs';
const LOAD_FAILURE_WORKER = fileURLToPath(
  new URL('./pool-preload-load-failure-worker.mjs', import.meta.url),
);
// Thrown by LOAD_FAILURE_WORKER's wasm instantiate.
const LOAD_FAILURE_MESSAGE = 'forced pool worker load failure (test)';
const DISPOSE_SYMBOL = Symbol.for('napi.rs.wasi.dispose');
const CRASH_DISPOSE_MESSAGE = 'cannot be disposed after a worker thread crashed';
const CASES = {
  default: { env: {} },
  'workers-4': { env: { ROLLDOWN_WORKER_THREADS: '4' }, workerThreads: 4 },
  single: { env: { ROLLDOWN_RUNTIME: 'single' }, flavor: 'CurrentThread' },
  'import-only': { env: {} },
  'load-failure': { env: {} },
  'in-flight': { env: {} },
};

const childIndex = process.argv.indexOf(CHILD_FLAG);
if (childIndex >= 0) {
  await runCase(process.argv[childIndex + 1], process.argv[childIndex + 2]);
} else {
  await runAll();
}

async function runAll() {
  const probe = await spawnCase('probe', '', {});
  const facts = JSON.parse(probe.output.match(/^PROBE (.*)$/m)?.[1] ?? 'null');
  if (!facts || facts.error) {
    throw new Error(`pool worker preload: rolldown failed to load\n${probe.output.trim()}`);
  }
  const threadless = facts.target === 'wasi';
  if (!threadless && facts.target !== 'wasi-threads') {
    console.log(`SKIP pool worker preload: needs a WASI build, got ${facts.target}`);
    return;
  }
  if (!threadless && facts.flavor !== 'MultiThread') {
    console.log(`SKIP pool worker preload: the default flavor is ${facts.flavor}`);
    return;
  }

  const workDir = mkdtempSync(path.join(os.tmpdir(), 'rolldown-wasi-pool-preload-'));
  const fixture = path.join(workDir, 'fixture');
  const failures = [];
  const started = Date.now();
  try {
    writeChain(fixture, MODULES);
    const cases = threadless ? { 'in-flight': { env: {}, flavor: 'CurrentThread' } } : CASES;
    for (const [mode, spec] of Object.entries(cases)) {
      for (let run = 1; run <= RUNS; run++) {
        const label = `${mode} #${run}`;
        const result = await spawnCase(mode, fixture, spec.env);
        const problem = judge(mode, spec, result);
        console.log(`${problem ? 'FAIL' : 'PASS'} ${label} (code ${result.code}, ${result.ms} ms)`);
        if (problem) {
          failures.push(label);
          console.log(`--- ${label}: ${problem}\n${result.output.trim()}\n---`);
        } else {
          console.log(`  ${result.output.match(/^RESULT (.*)$/m)?.[1]}`);
        }
      }
    }
  } finally {
    rmSync(workDir, { force: true, recursive: true });
  }
  console.log(`${facts.target} pool worker preload finished in ${Date.now() - started} ms`);
  if (failures.length > 0) {
    throw new Error(`${facts.target} pool worker preload failed: ${failures.join(', ')}`);
  }
}

function judge(mode, spec, result) {
  if (result.timedOut) return `still alive after ${CASE_TIMEOUT_MS} ms (hang)`;
  if (result.signal) return `killed by signal ${result.signal}`;
  if (result.code !== 0) return `exited with code ${result.code}`;
  const line = result.output.match(/^RESULT (.*)$/m);
  if (!line) return 'no RESULT line';
  const facts = JSON.parse(line[1]);
  if (facts.errors.length > 0) return `uncaught errors: ${JSON.stringify(facts.errors)}`;

  const flavor = spec.flavor ?? 'MultiThread';
  if (facts.flavor !== flavor) return `flavor ${facts.flavor}, expected ${flavor}`;
  const preloaded = flavor === 'MultiThread' ? (spec.workerThreads ?? DEFAULT_WORKER_THREADS) : 0;
  // No Worker at all under MultiThread: the loader carries no preload (a cli or
  // emnapi that lost it, or an addon without the pool-size export).
  if (flavor === 'MultiThread' && facts.afterImport === 0) {
    return `no pool Worker right after the import under MultiThread, expected ${preloaded}: the loader did not preload the pool`;
  }
  if (facts.afterImport !== preloaded) {
    return `${facts.afterImport} pool Workers right after the import, expected ${preloaded}`;
  }
  if (mode === 'import-only') {
    const exitMs = result.closedAt - result.lineAt.get('RESULT');
    return exitMs <= IMPORT_ONLY_EXIT_MS
      ? null
      : `exited ${exitMs} ms after the import, expected within ${IMPORT_ONLY_EXIT_MS} ms`;
  }
  if (mode === 'load-failure') {
    if (facts.failedExited !== preloaded) {
      return `${facts.failedExited} of ${preloaded} failing Workers exited`;
    }
    if (facts.crashFlag !== 1) return `loader crash flag ${facts.crashFlag}, expected 1`;
    // emnapi replaces each failed idle preload with a fresh, unloaded Worker, so the
    // pool keeps its size and the builds load those.
    const replaced = facts.beforeBuilds - facts.afterImport;
    if (replaced !== preloaded) {
      return `${replaced} pool Workers replaced the failed preloads before the builds, expected ${preloaded}`;
    }
    if (facts.byBuilds !== 0) {
      return `the builds created ${facts.byBuilds} more pool Workers, expected 0 (they take the replacements)`;
    }
    if (facts.builds !== 'ok') return `builds: ${facts.builds}`;
    // The load failure latched the binding, although the builds ran on fresh
    // Workers: the public disposer rejects with it as the cause.
    const dispose = facts.dispose;
    if (!dispose?.message?.includes(CRASH_DISPOSE_MESSAGE)) {
      return `disposer: ${JSON.stringify(dispose)}, expected a rejection "${CRASH_DISPOSE_MESSAGE}"`;
    }
    if (dispose.cause !== LOAD_FAILURE_MESSAGE) {
      return `disposer cause ${JSON.stringify(dispose.cause)}, expected "${LOAD_FAILURE_MESSAGE}"`;
    }
    return null;
  }
  if (mode === 'in-flight') {
    const stranded = facts.work.filter(
      (outcome) => outcome !== 'fulfilled' && !outcome.startsWith('rejected:AbortError'),
    );
    if (stranded.length > 0) {
      return `transform() over the disposer: ${JSON.stringify(stranded)}, expected fulfilled or AbortError`;
    }
    return judgeDispose(facts);
  }
  if (flavor === 'MultiThread' && facts.byBuilds !== 0) {
    return `the first builds created ${facts.byBuilds} more pool Workers, expected 0`;
  }
  return judgeBuilds(facts);
}

function judgeBuilds(facts) {
  return facts.builds === 'ok' ? null : `builds: ${facts.builds}`;
}

// The disposer terminated idle Workers. That is no crash: the binding is not
// latched, so the public disposer resolves.
function judgeDispose(facts) {
  if (facts.dispose !== 'resolved') {
    return `disposer: ${JSON.stringify(facts.dispose)}, expected it to resolve`;
  }
  // The flag lives in the pool Workers' data: without a Worker there is none.
  const crashFlag = facts.poolWorkers > 0 ? 0 : null;
  if (facts.crashFlag !== crashFlag) {
    return `loader crash flag ${facts.crashFlag}, expected ${crashFlag}`;
  }
  if (facts.exitedAfterDispose !== facts.poolWorkers) {
    return `${facts.exitedAfterDispose} of ${facts.poolWorkers} pool Workers exited after the disposer`;
  }
  return null;
}

function spawnCase(mode, fixture, runtimeEnv) {
  const env = { ...process.env, ...runtimeEnv };
  for (const name of ['ROLLDOWN_RUNTIME', 'ROLLDOWN_WORKER_THREADS']) {
    if (!(name in runtimeEnv)) delete env[name];
  }
  const started = Date.now();
  const child = spawn(
    process.execPath,
    [fileURLToPath(import.meta.url), CHILD_FLAG, mode, fixture],
    { env, stdio: ['ignore', 'pipe', 'pipe'] },
  );
  let output = '';
  const lineAt = new Map();
  child.stdout.on('data', (chunk) => {
    output += chunk;
    for (const tag of ['RESULT']) {
      if (!lineAt.has(tag) && output.includes(`${tag} `)) lineAt.set(tag, Date.now());
    }
  });
  child.stderr.on('data', (chunk) => (output += chunk));
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    child.kill('SIGKILL');
  }, CASE_TIMEOUT_MS);
  return new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => {
      clearTimeout(timer);
      const closedAt = Date.now();
      resolve({ code, signal, timedOut, output, lineAt, closedAt, ms: closedAt - started });
    });
  });
}

function writeChain(dir, count) {
  mkdirSync(dir, { recursive: true });
  for (let i = 0; i < count; i++) {
    const source =
      i < count - 1
        ? `import { f${i + 1} } from './m${i + 1}.js';\nexport function f${i}() { return f${i + 1}() + ${i}; }\n`
        : `export function f${i}() { return 1; }\n`;
    writeFileSync(path.join(dir, `m${i}.js`), source);
  }
}

// Child side. Output goes through writeSync: the process may die right after.
function say(text) {
  writeSync(1, `${text}\n`);
}

async function runCase(mode, fixture) {
  const errors = [];
  process.on('uncaughtException', (error) => errors.push(`uncaught: ${error?.message}`));
  process.on('unhandledRejection', (error) => errors.push(`unhandled: ${error?.message}`));

  // The loader reads `Worker` off `node:worker_threads` when it loads, so this wrap
  // must be in place before rolldown is imported.
  const poolWorkers = [];
  let crashFlag;
  // load-failure: every pool Worker created during the import (the preloaded ones)
  // fails to load; the ones the builds create later load normally.
  let failPoolWorkers = mode === 'load-failure';
  const OriginalWorker = workerThreads.Worker;
  workerThreads.Worker = class extends OriginalWorker {
    constructor(filename, options = {}) {
      const file = filename instanceof URL ? fileURLToPath(filename) : String(filename);
      const isPool = file.endsWith(POOL_WORKER_FILE);
      const fail = isPool && failPoolWorkers;
      if (isPool) {
        crashFlag ??= options.workerData?.crashFlag;
      }
      if (fail) {
        options = {
          ...options,
          workerData: { ...options.workerData, __poolPreloadTarget: file },
        };
        filename = LOAD_FAILURE_WORKER;
      }
      super(filename, options);
      if (isPool) {
        const record = { fail, exited: false };
        this.once('exit', () => (record.exited = true));
        poolWorkers.push(record);
      }
    }
  };
  if (mode === 'probe') {
    const facts = {};
    try {
      const { getRuntimeSupport } = await import('rolldown/experimental');
      Object.assign(facts, runtimeFacts(getRuntimeSupport()));
    } catch (error) {
      facts.error = String(error?.message ?? error);
    }
    say(`PROBE ${JSON.stringify(facts)}`);
    return;
  }

  // The binding loads while rolldown/experimental evaluates: the loader constructs
  // the preloaded pool Workers right then, synchronously.
  const { getRuntimeSupport } = await import('rolldown/experimental');
  failPoolWorkers = false;
  const afterImport = poolWorkers.length;
  const { flavor } = runtimeFacts(getRuntimeSupport());
  const facts = { flavor, afterImport, errors };
  const readCrashFlag = () => (crashFlag instanceof Int32Array ? Atomics.load(crashFlag, 0) : null);
  const { rolldown } = await import('rolldown');

  if (mode === 'import-only') {
    say(`RESULT ${JSON.stringify(facts)}`);
    return;
  }

  // Read after the public disposer, which terminates the remaining Workers too.
  const readDisposeFacts = async () => {
    facts.crashFlag = readCrashFlag();
    await waitUntil(() => poolWorkers.every((worker) => worker.exited));
    facts.poolWorkers = poolWorkers.length;
    facts.exitedAfterDispose = poolWorkers.filter((worker) => worker.exited).length;
  };

  if (mode === 'in-flight') {
    // transform() is rolldown's napi Task, so these queue napi_async_work, and each is
    // still owed its completion callback when the disposer starts.
    const { transform } = await import('rolldown/utils');
    let entered, release;
    const inBinding = new Promise((resolve) => (entered = resolve));
    const held = new Promise((resolve) => (release = resolve));
    const bundle = await rolldown({
      input: 'held',
      plugins: [
        {
          name: 'pool-preload-hold',
          resolveId: (id) => id,
          async load() {
            entered();
            await held;
            return 'export default 1';
          },
        },
      ],
    });
    const outcome = (promise) =>
      promise.then(
        () => 'fulfilled',
        (error) => `rejected:${error?.name}: ${String(error?.message).slice(0, 200)}`,
      );
    const build = outcome(bundle.generate({ format: 'esm' }));
    await inBinding;
    const source = 'export const value: number = 1;\n'.repeat(400);
    const work = Array.from({ length: 8 }, (_, i) => outcome(transform(`t${i}.ts`, source)));
    const disposed = findDisposer()();
    release();
    try {
      await disposed;
      facts.dispose = 'resolved';
    } catch (error) {
      facts.dispose = { message: error?.message, cause: error?.cause?.message };
    }
    facts.build = await build;
    facts.work = await Promise.all(work);
    await readDisposeFacts();
    say(`RESULT ${JSON.stringify(facts)}`);
    return;
  }

  if (mode === 'load-failure') {
    const deadline = Date.now() + LOAD_FAILURE_EXIT_TIMEOUT_MS;
    while (poolWorkers.some((worker) => worker.fail && !worker.exited)) {
      if (Date.now() > deadline) break;
      await new Promise((resolve) => setTimeout(resolve, 5));
    }
    // One more turn, so emnapi's rejection handlers (and the preload's drop) ran.
    await new Promise((resolve) => setTimeout(resolve, 20));
    facts.failedExited = poolWorkers.filter((worker) => worker.fail && worker.exited).length;
    facts.crashFlag = readCrashFlag();
  }

  const beforeBuilds = poolWorkers.length;
  facts.beforeBuilds = beforeBuilds;
  const plugin = {
    name: 'pool-preload-js-plugin',
    // Called from the pool threads through a threadsafe function on every module.
    transform(code) {
      return { code: `${code}\nconsole.log('seen by a JS plugin');\n` };
    },
  };
  const once = async () => {
    const bundle = await rolldown({ input: path.join(fixture, 'm0.js'), plugins: [plugin] });
    try {
      const { output } = await bundle.generate({ format: 'esm' });
      assert.match(output[0].code, /seen by a JS plugin/);
    } finally {
      await bundle.close();
    }
  };
  try {
    await Promise.all(Array.from({ length: CONCURRENCY }, once));
    facts.builds = 'ok';
  } catch (error) {
    facts.builds = `failed: ${String(error?.message).slice(0, 300)}`;
  }
  facts.byBuilds = poolWorkers.length - beforeBuilds;
  if (mode === 'load-failure') {
    try {
      await findDisposer()();
      facts.dispose = 'resolved';
    } catch (error) {
      facts.dispose = { message: error?.message, cause: error?.cause?.message };
    }
  }
  say(`RESULT ${JSON.stringify(facts)}`);
}

// The threaded loader publishes the disposer on its own exports. Take it from the
// instance the builds ran on (the require cache), never from a second copy.
function findDisposer() {
  const require = createRequire(import.meta.url);
  const found = Object.values(require.cache).filter(
    (entry) => typeof entry?.exports?.[DISPOSE_SYMBOL] === 'function',
  );
  assert.equal(found.length, 1, 'expected exactly one loaded WASI binding with a disposer');
  return found[0].exports[DISPOSE_SYMBOL];
}

// The public API reports workflow support; `dev` needs MultiThread, and parallel
// plugins run only on the native binary.
function runtimeFacts(support) {
  return {
    target: support.parallelPlugins ? 'native' : support.threadlessWasi ? 'wasi' : 'wasi-threads',
    flavor: support.dev ? 'MultiThread' : 'CurrentThread',
  };
}

async function waitUntil(done) {
  const deadline = Date.now() + WAIT_TIMEOUT_MS;
  while (!done() && Date.now() <= deadline) {
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
}
