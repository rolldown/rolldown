// Worker-crash latch for the threaded WASI artifact, under MultiThread with 4 workers.
//
// When a pool worker's wasm thread dies, any teardown that re-enters wasm waits for
// the dead thread's work to go idle, and that never happens. Without the latch in the
// generated threaded Node loader:
// - the exit listener blocks the main thread in a raw atomic wait, so the process
//   never exits and ignores SIGTERM;
// - the public disposer, binding[Symbol.for('napi.rs.wasi.dispose')](), drains into
//   the cleanup barrier and its promise never settles.
// See internal-docs/async-runtime/implementation.md (section 7, Loaders).
//
// Each run is a child process that preloads crash-injector-preload.mjs: in the pool
// workers, the CRASH_AT-th `fd_read` call throws a RuntimeError, the same path a real
// trap takes. 16 concurrent builds of a 300-module chain make sure a worker gets there.
//   no-handler  no uncaughtException handler: must exit non-zero
//   dispose     handles the crash, then awaits the disposer: must reject with the
//               crash as its cause (latched: a second call returns the same
//               promise), then the process must exit on its own with code 0
//               (no process.exit): the disposer unrefs emnapi's waiting-request
//               port, which the dead worker's unfinished requests keep ref'd
//   in-flight   the crash lands while a disposal is already running: once the builds
//               are reading files, the child calls the disposer, then arms the
//               injector, and the next `sched_yield` in any pool worker throws (the
//               workers yield while the runtime shuts down). The disposal polls for
//               runtime work the dead worker never finishes; it must stop and reject
//               like the dispose case within IN_FLIGHT_SETTLE_MS (every call returns
//               that one promise), and the process must exit on its own with code 0
//   control     no injection: the builds pass and the process exits 0
// In both disposer cases the rejection's cause is the injected error (name and
// message, taken from the worker's shared crash report when its 'error' event is
// lost), and `workerThreadId` is the threadId of a worker that crashed.
// A crash run still alive after CRASH_CASE_TIMEOUT_MS is killed and counts as a hang.
// Skips (exit 0) unless the artifact is the threaded WASI one and MultiThread works.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, writeSync } from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { threadId } from 'node:worker_threads';

const CHILD_FLAG = '--run-worker-crash-latch-case';
const MODULES = 300;
const CONCURRENCY = 16;
const RUNS = 3;
// A crash case must be over well within this; the control case gets the stress
// script's budget, since 16 debug-wasm builds on a slow runner can take a while.
const CRASH_CASE_TIMEOUT_MS = 15_000;
// After the disposer settles, a dispose child that is still alive reports its live
// handles this often, so a hang names what holds the event loop open.
const LIVE_HANDLES_REPORT_MS = 2_000;
const CONTROL_TIMEOUT_MS = 60_000;
const CRASH_IMPORT = 'wasi_snapshot_preview1.fd_read';
const CRASH_AT = '20';
// In-flight case: the import that throws once armed (the pool workers' runtime
// yields while the builds run), how many `fd_read` calls must have happened before
// the disposer is called, and how long the disposal may take to settle after that.
const IN_FLIGHT_CRASH_IMPORT = 'wasi_snapshot_preview1.sched_yield';
const IN_FLIGHT_READS_BEFORE_DISPOSE = 50;
const IN_FLIGHT_PROGRESS_TIMEOUT_MS = 10_000;
const IN_FLIGHT_CRASH_WAIT_MS = 5_000;
const IN_FLIGHT_SETTLE_MS = 5_000;
const CRASH_CONTROL = Symbol.for('rolldown.test.crashControl');
const LOADER_CRASH_FLAG = Symbol.for('rolldown.test.loaderCrashFlag');
const INJECTED_ERROR_NAME = 'RuntimeError';
const MULTI_THREAD_ENV = { ROLLDOWN_RUNTIME: 'multi', ROLLDOWN_WORKER_THREADS: '4' };
const DISPOSE_SYMBOL = Symbol.for('napi.rs.wasi.dispose');
const CRASH_DISPOSE_MESSAGE = 'cannot be disposed after a worker thread crashed';
const CASES = ['no-handler', 'dispose', 'in-flight', 'control'];

const childIndex = process.argv.indexOf(CHILD_FLAG);
if (childIndex >= 0) {
  await runCase(process.argv[childIndex + 1], process.argv[childIndex + 2]);
} else {
  await runAll();
}

async function runAll() {
  const { target, error: loadError } = await probe({});
  if (loadError) {
    throw new Error(`worker crash latch: rolldown failed to load: ${loadError}`);
  }
  if (target !== 'wasi-threads') {
    console.log(`SKIP worker crash latch: needs the threaded WASI build, got target ${target}`);
    return;
  }
  const { flavor, error } = await probe(MULTI_THREAD_ENV);
  if (flavor !== 'MultiThread') {
    console.log(`SKIP worker crash latch: MultiThread is not available (${error ?? flavor})`);
    return;
  }

  const workDir = mkdtempSync(path.join(os.tmpdir(), 'rolldown-wasi-crash-latch-'));
  const fixture = path.join(workDir, 'fixture');
  const failures = [];
  const started = Date.now();
  try {
    writeChain(fixture, MODULES);
    for (const mode of CASES) {
      for (let run = 1; run <= RUNS; run++) {
        const label = `${mode} #${run}`;
        const crashLog = path.join(workDir, `${mode}-${run}.crash.log`);
        const result = await spawnCase(mode, fixture, {
          runtimeEnv: MULTI_THREAD_ENV,
          inject: mode !== 'control',
          crashLog,
        });
        const crashedThreads = [...readText(crashLog).matchAll(/^t(\d+) CRASH in /gm)].map(
          (match) => Number(match[1]),
        );
        const problem = judge(mode, result, crashedThreads);
        console.log(`${problem ? 'FAIL' : 'PASS'} ${label} (code ${result.code}, ${result.ms} ms)`);
        if (problem) {
          failures.push(label);
          console.log(`--- ${label}: ${problem}\n${result.output.trim()}\n---`);
        }
      }
    }
  } finally {
    rmSync(workDir, { force: true, recursive: true });
  }
  console.log(`threaded WASI worker crash latch finished in ${Date.now() - started} ms`);
  if (failures.length > 0) {
    throw new Error(`threaded WASI worker crash latch failed: ${failures.join(', ')}`);
  }
}

async function probe(runtimeEnv) {
  const result = await spawnCase('probe', '', { runtimeEnv });
  const line = result.output.match(/^PROBE (.*)$/m);
  if (!line) {
    throw new Error(`worker crash latch: the probe failed\n${result.output.trim()}`);
  }
  return JSON.parse(line[1]);
}

function judge(mode, result, crashedThreads) {
  const disposes = mode === 'dispose' || mode === 'in-flight';
  if (result.timedOut) {
    if (disposes && result.output.includes('DISPOSE_OK')) {
      const reports = [...result.output.matchAll(/^STILL_ALIVE (.*)$/gm)];
      const handles = reports.length > 0 ? reports[reports.length - 1][1] : 'not reported';
      return (
        `the disposer rejected as expected, but the process did not exit on its own ` +
        `within ${result.timeoutMs} ms; live handles (process.getActiveResourcesInfo()): ${handles}`
      );
    }
    if (disposes && result.output.includes('DISPOSE_PENDING')) {
      return `the disposer never settled after the crash (hang, killed after ${result.timeoutMs} ms)`;
    }
    return `still alive after ${result.timeoutMs} ms (hang)`;
  }
  if (result.signal) {
    return `killed by signal ${result.signal}`;
  }
  if (mode === 'control') {
    if (result.code !== 0 || !result.output.includes('BUILDS_OK')) {
      return `exited with code ${result.code} without BUILDS_OK`;
    }
    return null;
  }
  if (crashedThreads.length === 0 || result.output.includes('BUILDS_OK')) {
    return 'the injected worker crash did not fire';
  }
  if (mode === 'no-handler') {
    return result.code === 0 ? 'exited with code 0 after an unhandled worker crash' : null;
  }
  if (!/^DISPOSE_REJECTED /m.test(result.output)) {
    return `the disposer did not reject (exit code ${result.code})`;
  }
  if (!result.output.includes('DISPOSE_OK')) {
    return (
      `the disposer's rejection failed its checks: latch message, injected cause, ` +
      `workerThreadId, case checks (exit code ${result.code})`
    );
  }
  const workerThreadId = Number(result.output.match(/^WORKER_THREAD_ID (\d+)$/m)?.[1]);
  if (!crashedThreads.includes(workerThreadId)) {
    return (
      `the rejection names worker thread ${workerThreadId}, but the crashed ` +
      `worker threads are ${JSON.stringify(crashedThreads)}`
    );
  }
  if (!result.output.includes('DISPOSE_SETTLED')) {
    return `exited with code ${result.code} before the disposer settled`;
  }
  return result.code === 0
    ? null
    : `exited on its own with code ${result.code} after the disposer settled, expected 0`;
}

function spawnCase(mode, fixture, { runtimeEnv, inject = false, crashLog }) {
  const env = { ...process.env, ...runtimeEnv };
  if (!('ROLLDOWN_RUNTIME' in runtimeEnv)) {
    delete env.ROLLDOWN_RUNTIME;
    delete env.ROLLDOWN_WORKER_THREADS;
  }
  delete env.ROLLDOWN_TEST_CRASH_IMPORT;
  delete env.ROLLDOWN_TEST_CRASH_ARMED;
  const args = [];
  if (inject) {
    env.ROLLDOWN_TEST_CRASH_IMPORT = mode === 'in-flight' ? IN_FLIGHT_CRASH_IMPORT : CRASH_IMPORT;
    env.ROLLDOWN_TEST_CRASH_AT = CRASH_AT;
    env.ROLLDOWN_TEST_CRASH_LOG = crashLog;
    if (mode === 'in-flight') {
      env.ROLLDOWN_TEST_CRASH_ARMED = '1';
    }
    args.push('--import', pathToFileURL(helper('crash-injector-preload.mjs')).href);
  }
  args.push(fileURLToPath(import.meta.url), CHILD_FLAG, mode, fixture);
  const started = Date.now();
  const child = spawn(process.execPath, args, { env, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  child.stdout.on('data', (chunk) => (output += chunk));
  child.stderr.on('data', (chunk) => (output += chunk));
  let timedOut = false;
  const timeoutMs = mode === 'control' ? CONTROL_TIMEOUT_MS : CRASH_CASE_TIMEOUT_MS;
  const timer = setTimeout(() => {
    timedOut = true;
    child.kill('SIGKILL');
  }, timeoutMs);
  return new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => {
      clearTimeout(timer);
      resolve({
        code,
        signal: timedOut ? null : signal,
        timedOut,
        timeoutMs,
        output,
        ms: Date.now() - started,
      });
    });
  });
}

function helper(file) {
  return fileURLToPath(new URL(`./${file}`, import.meta.url));
}

function readText(file) {
  try {
    return readFileSync(file, 'utf8');
  } catch {
    return '';
  }
}

function writeChain(dir, count) {
  // A linear import chain m0 -> m1 -> ...: every build reads all the files, so the
  // pool workers call fd_read well past CRASH_AT.
  mkdirSync(dir, { recursive: true });
  const pad = 'a'.repeat(600);
  for (let i = 0; i < count; i++) {
    const source =
      i < count - 1
        ? `import { f${i + 1} } from './m${i + 1}.js';\n` +
          `export function f${i}() { return f${i + 1}() + ${i} + '${pad}'.length; }\n`
        : `export function f${i}() { return 1; }\n`;
    writeFileSync(path.join(dir, `m${i}.js`), source);
  }
}

// Child side. Output goes through writeSync: the process may die right after.
function say(text) {
  writeSync(1, `${text}\n`);
}

async function runCase(mode, fixture) {
  if (mode === 'probe') {
    const facts = {};
    try {
      const { getAsyncRuntimeConfig, getRuntimeCapabilities } =
        await import('rolldown/experimental');
      facts.target = getRuntimeCapabilities().target;
      facts.flavor = getAsyncRuntimeConfig().flavor;
    } catch (error) {
      facts.error = String(error?.message ?? error);
    }
    say(`PROBE ${JSON.stringify(facts)}`);
    return;
  }

  let crash;
  let onCrash = () => {};
  const crashed = new Promise((resolve) => (onCrash = resolve));
  if (mode === 'dispose' || mode === 'in-flight') {
    process.on('uncaughtException', (error) => {
      if (crash) return;
      crash = error;
      say(`CAUGHT ${error?.message}`);
      onCrash();
    });
  }

  const { rolldown } = await import('rolldown');
  const { getAsyncRuntimeConfig } = await import('rolldown/experimental');
  const config = getAsyncRuntimeConfig();
  assert.equal(config.flavor, 'MultiThread');
  assert.equal(config.workerThreads, Number(MULTI_THREAD_ENV.ROLLDOWN_WORKER_THREADS));

  const once = async () => {
    const bundle = await rolldown({ input: path.join(fixture, 'm0.js') });
    try {
      await bundle.generate({ format: 'esm' });
    } finally {
      await bundle.close();
    }
  };
  const builds = Promise.all(Array.from({ length: CONCURRENCY }, once)).then(() =>
    say('BUILDS_OK'),
  );
  if (mode === 'in-flight') {
    await disposeInFlight(builds, () => crash);
    return;
  }
  if (mode !== 'dispose') {
    await builds;
    return;
  }

  // The builds that ran on the dead worker may never settle, so wait for whichever
  // comes first. Builds that all pass mean the crash did not fire (the parent fails
  // the run); builds that fail still wait for the crash report.
  const first = await Promise.race([
    crashed.then(() => 'crash'),
    builds.then(
      () => 'builds',
      (error) => {
        say(`BUILDS_ERROR ${error?.message}`);
        return 'error';
      },
    ),
  ]);
  if (first === 'builds') {
    return;
  }
  await crashed;

  let dispose;
  try {
    dispose = findDisposer();
  } catch (error) {
    say(`NO_DISPOSER ${error?.message}`);
    process.exit(1);
  }
  const disposal = dispose();
  const again = dispose();
  try {
    await disposal;
    say('DISPOSE_RESOLVED');
  } catch (error) {
    reportCrashRejection(error, CRASH_IMPORT, [again === disposal]);
  }
  settleAndExitNaturally();
}

// The in-flight case: the disposal is already running when the worker dies.
async function disposeInFlight(builds, getCrash) {
  const control = globalThis[CRASH_CONTROL];
  assert.ok(control instanceof Int32Array, 'the crash injector control buffer is missing');
  let buildsDone = false;
  builds.then(
    () => (buildsDone = true),
    (error) => {
      buildsDone = true;
      say(`BUILDS_ERROR ${error?.message}`);
    },
  );
  // Call the disposer only once the builds are reading files, so its async-work
  // drain has work to wait for that the dead worker will never finish.
  const deadline = Date.now() + IN_FLIGHT_PROGRESS_TIMEOUT_MS;
  while (Atomics.load(control, 1) < IN_FLIGHT_READS_BEFORE_DISPOSE) {
    if (buildsDone || Date.now() > deadline) {
      say(`NO_PROGRESS reads=${Atomics.load(control, 1)} builds-done=${buildsDone}`);
      process.exit(1);
    }
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  if (getCrash() !== undefined || Atomics.load(control, 0) !== 0) {
    say('CRASHED_BEFORE_DISPOSE');
    process.exit(1);
  }
  let dispose;
  try {
    dispose = findDisposer();
  } catch (error) {
    say(`NO_DISPOSER ${error?.message}`);
    process.exit(1);
  }
  const loaderCrashFlag = globalThis[LOADER_CRASH_FLAG];
  assert.ok(loaderCrashFlag instanceof Int32Array, "the loader's crash flag was not captured");
  const disposal = dispose();
  const again = dispose();
  // The disposal is in flight (its promise is out); only now can a worker crash.
  Atomics.store(control, 0, 1);
  // Stay in JavaScript until the dying worker has raised the loader's crash flag,
  // so the crash is visible before the disposal's next poll turn calls into wasm.
  // The loader checks the flag at each turn; it cannot help a call that is already
  // inside wasm and waiting on something the dead worker held, so this case does
  // not let the crash land during such a call.
  const crashWaitEnd = Date.now() + IN_FLIGHT_CRASH_WAIT_MS;
  while (Atomics.load(loaderCrashFlag, 0) === 0 && Date.now() < crashWaitEnd) {
    // Busy-wait: Atomics.wait is not allowed on the main thread.
  }
  say(
    `ARMED reads=${Atomics.load(control, 1)} fired=${Atomics.load(control, 0) === 2} ` +
      `crash-flag=${Atomics.load(loaderCrashFlag, 0)}`,
  );
  // Reported only while something else still holds the event loop open.
  const pending = setTimeout(() => {
    say(`DISPOSE_PENDING fired=${Atomics.load(control, 0) === 2} after ${IN_FLIGHT_SETTLE_MS} ms`);
  }, IN_FLIGHT_SETTLE_MS);
  pending.unref();
  const started = Date.now();
  try {
    await disposal;
    say(`DISPOSE_RESOLVED fired=${Atomics.load(control, 0) === 2}`);
  } catch (error) {
    const ms = Date.now() - started;
    say(`DISPOSE_SETTLE_MS ${ms}`);
    const later = dispose();
    reportCrashRejection(error, IN_FLIGHT_CRASH_IMPORT, [
      again === disposal,
      later === disposal,
      ms <= IN_FLIGHT_SETTLE_MS,
    ]);
    later.catch(() => {});
  } finally {
    clearTimeout(pending);
  }
  settleAndExitNaturally();
}

// The disposer's rejection after a crash: the latch error, with the injected
// worker error as its cause and the crashed worker's threadId. `checks` are the
// case's own conditions (latched promise, bounded settle time).
function reportCrashRejection(error, crashImport, checks) {
  const cause = error?.cause;
  const workerThreadId = error?.workerThreadId;
  say(
    `DISPOSE_REJECTED ${error?.message} | cause=${cause?.name}: ${cause?.message} | ` +
      `workerThreadId=${workerThreadId} | checks=${JSON.stringify(checks)}`,
  );
  if (Number.isInteger(workerThreadId)) {
    say(`WORKER_THREAD_ID ${workerThreadId}`);
  }
  if (
    String(error?.message).includes(CRASH_DISPOSE_MESSAGE) &&
    cause?.name === INJECTED_ERROR_NAME &&
    cause?.message === `forced worker crash in ${crashImport} (test)` &&
    Number.isInteger(workerThreadId) &&
    workerThreadId > 0 &&
    workerThreadId !== threadId &&
    checks.every(Boolean)
  ) {
    say('DISPOSE_OK');
  }
}

function settleAndExitNaturally() {
  say('DISPOSE_SETTLED');
  // No process.exit: the event loop must drain by itself. The crash path cannot
  // destroy the emnapi context, so the requests the dead worker never finished
  // keep its waiting-request count above zero; the disposer unrefs that counter's
  // port. The natural exit also runs the loader's exit listener after the crash
  // disposal. If anything still holds the loop open, name it for the parent; the
  // unref'd timer never keeps the process alive by itself.
  setInterval(() => {
    say(`STILL_ALIVE ${JSON.stringify(process.getActiveResourcesInfo())}`);
  }, LIVE_HANDLES_REPORT_MS).unref();
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
