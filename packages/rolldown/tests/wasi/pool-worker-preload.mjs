// Pool worker preload in the threaded WASI Node loader.
//
// The build inserts a preload into the generated `rolldown-binding.wasi.cjs`: right
// after the binding loads, one Worker per configured MultiThread worker goes into
// emnapi's reuse pool and starts loading the wasm, so the first build's thread spawns
// take a Worker that is already booting instead of creating one.
// See internal-docs/async-runtime/implementation.md (section 13, "Pool worker preload").
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
//                 the loader's crash flag and exits; they leave the pool, so the
//                 next build creates N fresh Workers and succeeds
// Every case fails on an uncaught exception or unhandled rejection, and a child
// still alive after CASE_TIMEOUT_MS is killed and counts as a hang.
// Skips (exit 0) unless the artifact is the threaded WASI one and MultiThread works.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, writeSync } from 'node:fs';
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
// How long the load-failure child waits for the failing Workers to exit.
const LOAD_FAILURE_EXIT_TIMEOUT_MS = 10_000;
const POOL_WORKER_FILE = 'wasi-worker.mjs';
const LOAD_FAILURE_WORKER = fileURLToPath(
  new URL('./pool-preload-load-failure-worker.mjs', import.meta.url),
);
const CASES = {
  default: { env: {} },
  'workers-4': { env: { ROLLDOWN_WORKER_THREADS: '4' }, workerThreads: 4 },
  single: { env: { ROLLDOWN_RUNTIME: 'single' }, flavor: 'CurrentThread' },
  'import-only': { env: {} },
  'load-failure': { env: {} },
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
  if (facts.target !== 'wasi-threads') {
    console.log(`SKIP pool worker preload: needs the threaded WASI build, got ${facts.target}`);
    return;
  }
  if (facts.flavor !== 'MultiThread') {
    console.log(`SKIP pool worker preload: the default flavor is ${facts.flavor}`);
    return;
  }

  const workDir = mkdtempSync(path.join(os.tmpdir(), 'rolldown-wasi-pool-preload-'));
  const fixture = path.join(workDir, 'fixture');
  const failures = [];
  const started = Date.now();
  try {
    writeChain(fixture, MODULES);
    for (const [mode, spec] of Object.entries(CASES)) {
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
  console.log(`threaded WASI pool worker preload finished in ${Date.now() - started} ms`);
  if (failures.length > 0) {
    throw new Error(`threaded WASI pool worker preload failed: ${failures.join(', ')}`);
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
  if (spec.workerThreads !== undefined && facts.workerThreads !== spec.workerThreads) {
    return `workerThreads ${facts.workerThreads}, expected ${spec.workerThreads}`;
  }
  const preloaded = flavor === 'MultiThread' ? facts.workerThreads : 0;
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
    // The failed Workers left the pool, so the build replaces each one.
    if (facts.byBuilds !== preloaded) {
      return `the builds created ${facts.byBuilds} pool Workers, expected ${preloaded} fresh ones`;
    }
    return facts.builds === 'ok' ? null : `builds: ${facts.builds}`;
  }
  if (flavor === 'MultiThread' && facts.byBuilds !== 0) {
    return `the first builds created ${facts.byBuilds} more pool Workers, expected 0`;
  }
  return facts.builds === 'ok' ? null : `builds: ${facts.builds}`;
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

  const { rolldown } = await import('rolldown');
  const { getAsyncRuntimeConfig } = await import('rolldown/experimental');
  failPoolWorkers = false;
  const afterImport = poolWorkers.length;
  const { flavor, workerThreads: configured } = getAsyncRuntimeConfig();
  const facts = { flavor, workerThreads: configured, afterImport, errors };

  if (mode === 'import-only') {
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
    facts.crashFlag = crashFlag instanceof Int32Array ? Atomics.load(crashFlag, 0) : null;
  }

  const beforeBuilds = poolWorkers.length;
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
  say(`RESULT ${JSON.stringify(facts)}`);
}
