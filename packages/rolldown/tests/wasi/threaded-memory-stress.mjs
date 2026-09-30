// Concurrency stress for the threaded WASI artifact, under CurrentThread
// (`ROLLDOWN_RUNTIME=single`), under the default MultiThread (2 workers), and under
// MultiThread with 4 workers. Each case sets its own flavor, so the lane's
// `ROLLDOWN_RUNTIME` does not change what this script runs.
//
// Without napi's `wasi_heap_sync` allocator lock these loads trapped with "memory access out of
// bounds": V8 keeps a stale shared-memory size on threads that did not run memory.grow,
// and bounds-checks memory.fill / memory.copy (and, on hosts without the wasm trap
// handler, every load and store) against it. Without the fix, one run of this script
// fails nearly every time, so one passing run pins the fix.
// See internal-docs/wasi-shared-memory-grow/design.md
//
// Each case runs in its own child process: a wasm trap kills the process, and after a
// worker crash the process can hang on exit, so the parent enforces a per-case timeout.
//
// Options (all optional; with none, every default case runs once):
//   --cases a,b          case ids (see CASES); `build-mt4` is not in the default set
//   --runs N             run each case N times (sequentially)
//   --node-flag F        pass a node flag to every child (repeatable), for example
//                        --wasm-enforce-bounds-checks or --disable-wasm-trap-handler. Node
//                        rejects these in NODE_OPTIONS, so they go on the command line.
//   --initial-pages P    create the loader's shared memory with P pages instead of the
//                        loader's 16384 (`min` = the module's declared minimum), so the heap
//                        must grow while the threads run
//   --hold-mib N         before the case, allocate N MiB inside wasm and keep it
//   --expect-grows E     `zero` or `some`: the number of memory.grow calls the heap-sync
//                        allocator made during the child's life
// Every child also checks the heap-sync invariant (no block ever ended past the
// allocating thread's refreshed size) through the module's `napi_wasm_heap_sync_stat`
// export (napi-rs), and prints its counters.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { readMemoryImport } from '../../../../scripts/wasi/wasm-sections.mjs';

const CHILD_FLAG = '--run-threaded-memory-stress-case';
const MODULES = 300;
const CONCURRENCY = 16;
const CASE_TIMEOUT_MS = Number(process.env.ROLLDOWN_WASI_STRESS_CASE_TIMEOUT_MS ?? 60_000);
const MIB = 1 << 20;

const CURRENT_THREAD = { name: 'CurrentThread', env: { ROLLDOWN_RUNTIME: 'single' } };
// No `ROLLDOWN_RUNTIME`: the artifact's default, MultiThread with 2 workers.
const DEFAULT_MULTI_THREAD = { name: 'MultiThread (default)', env: {} };
const MULTI_THREAD = {
  name: 'MultiThread w4',
  env: { ROLLDOWN_RUNTIME: 'multi', ROLLDOWN_WORKER_THREADS: '4' },
};

// `bundle`: CONCURRENCY builds x 2 waves with an async JS plugin (one threadsafe-function
// call per hook, so blocks cross between the workers and the JS thread).
// `build`: CONCURRENCY builds, one wave, no plugin (only the Rust workers allocate).
// `parse` / `transform`: CONCURRENCY concurrent passes over every module, 3 times.
// parse() and transform() run on emnapi's async-work pool, not on the scheduler, so
// the default MultiThread case adds only a bundle load.
const CASES = {
  'bundle-ct': { mode: 'bundle', runtime: CURRENT_THREAD },
  'bundle-mt': { mode: 'bundle', runtime: DEFAULT_MULTI_THREAD },
  'bundle-mt4': { mode: 'bundle', runtime: MULTI_THREAD },
  'parse-ct': { mode: 'parse', runtime: CURRENT_THREAD },
  'parse-mt4': { mode: 'parse', runtime: MULTI_THREAD },
  'transform-ct': { mode: 'transform', runtime: CURRENT_THREAD },
  'transform-mt4': { mode: 'transform', runtime: MULTI_THREAD },
  'build-mt4': { mode: 'build', runtime: MULTI_THREAD },
};
const DEFAULT_CASES = [
  'bundle-ct',
  'bundle-mt',
  'bundle-mt4',
  'parse-ct',
  'parse-mt4',
  'transform-ct',
  'transform-mt4',
];

const childIndex = process.argv.indexOf(CHILD_FLAG);
if (childIndex >= 0) {
  await runCase(process.argv[childIndex + 1], process.argv[childIndex + 2]);
} else {
  await runAll(parseOptions());
}

function parseOptions() {
  const { values } = parseArgs({
    options: {
      cases: { type: 'string' },
      runs: { type: 'string', default: '1' },
      'node-flag': { type: 'string', multiple: true, default: [] },
      'initial-pages': { type: 'string' },
      'hold-mib': { type: 'string' },
      'expect-grows': { type: 'string' },
    },
  });
  const cases = values.cases ? values.cases.split(',') : DEFAULT_CASES;
  for (const id of cases) assert.ok(CASES[id], `unknown case ${id}`);
  const runs = Number(values.runs);
  assert.ok(Number.isInteger(runs) && runs > 0, `--runs must be a positive integer`);
  const initialPages = values['initial-pages'];
  assert.ok(
    initialPages === undefined || initialPages === 'min' || /^\d+$/.test(initialPages),
    `--initial-pages must be a page count or "min"`,
  );
  const holdMiB = values['hold-mib'];
  assert.ok(holdMiB === undefined || /^\d+$/.test(holdMiB), `--hold-mib must be a MiB count`);
  const expectGrows = values['expect-grows'];
  assert.ok(
    expectGrows === undefined || expectGrows === 'zero' || expectGrows === 'some',
    `--expect-grows must be "zero" or "some"`,
  );
  return { cases, runs, nodeFlags: values['node-flag'], initialPages, holdMiB, expectGrows };
}

async function runAll({ cases, runs, nodeFlags, initialPages, holdMiB, expectGrows }) {
  const fixture = mkdtempSync(path.join(os.tmpdir(), 'rolldown-wasi-stress-'));
  const failures = [];
  const started = Date.now();
  const childEnv = {
    ROLLDOWN_WASI_STRESS_INITIAL_PAGES: initialPages,
    ROLLDOWN_WASI_STRESS_HOLD_MIB: holdMiB,
    ROLLDOWN_WASI_STRESS_EXPECT_GROWS: expectGrows,
  };
  const setup = [
    nodeFlags.length > 0 && `node flags: ${nodeFlags.join(' ')}`,
    initialPages && `initial pages: ${initialPages}`,
    holdMiB && `hold: ${holdMiB} MiB`,
    expectGrows && `expect grows: ${expectGrows}`,
  ].filter(Boolean);
  if (setup.length > 0) console.log(setup.join(', '));
  try {
    writeChain(fixture, MODULES);
    for (const id of cases) {
      const { mode, runtime } = CASES[id];
      let passed = 0;
      for (let run = 1; run <= runs; run++) {
        const label = `${mode} ${runtime.name}${runs > 1 ? ` #${run}` : ''}`;
        const result = await spawnCase(mode, fixture, { ...runtime.env, ...childEnv }, nodeFlags);
        const ok = result.code === 0 && !result.timedOut && result.output.includes('STRESS_OK');
        const counters = result.output.match(/^HEAP_SYNC .*$/m)?.[0] ?? '';
        console.log(`${ok ? 'PASS' : 'FAIL'} ${label} (${result.ms} ms) ${counters}`);
        if (ok) {
          passed++;
        } else {
          failures.push(label);
          const reason = result.timedOut
            ? `timed out after ${CASE_TIMEOUT_MS} ms`
            : `exited with ${result.signal ? `signal ${result.signal}` : `code ${result.code}`}`;
          console.log(`--- ${label}: ${reason}\n${result.output.trim()}\n---`);
        }
      }
      if (runs > 1) console.log(`${id}: ${passed}/${runs} passed`);
    }
  } finally {
    rmSync(fixture, { force: true, recursive: true });
  }
  console.log(`threaded WASI memory stress finished in ${Date.now() - started} ms`);
  if (failures.length > 0) {
    throw new Error(`threaded WASI memory stress failed: ${failures.join(', ')}`);
  }
}

function spawnCase(mode, fixture, caseEnv, nodeFlags) {
  const env = { ...process.env };
  delete env.ROLLDOWN_RUNTIME;
  delete env.ROLLDOWN_WORKER_THREADS;
  for (const [key, value] of Object.entries(caseEnv)) {
    if (value === undefined) delete env[key];
    else env[key] = value;
  }
  const started = Date.now();
  const child = spawn(
    process.execPath,
    [...nodeFlags, fileURLToPath(import.meta.url), CHILD_FLAG, mode, fixture],
    { env, stdio: ['ignore', 'pipe', 'pipe'] },
  );
  let output = '';
  child.stdout.on('data', (chunk) => (output += chunk));
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
      resolve({ code, signal, timedOut, output, ms: Date.now() - started });
    });
  });
}

function writeChain(dir, count) {
  // A linear import chain m0 -> m1 -> ... ; each module carries a 600-char string so the
  // builds move real bytes and the heap grows while threads allocate.
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

// The wasm file the threaded loader picks (it prefers the `.debug.wasm` next to it).
function threadedWasmPath() {
  const dist = path.dirname(fileURLToPath(import.meta.resolve('rolldown')));
  const debug = path.join(dist, 'rolldown-binding.wasm32-wasi.debug.wasm');
  return existsSync(debug) ? debug : path.join(dist, 'rolldown-binding.wasm32-wasi.wasm');
}

// Test-only hooks, installed before the loader runs: capture the module's exports (for the
// heap-sync counters and `malloc`) and, when asked, give the loader's shared memory a
// smaller initial size. Neither changes how the binding behaves.
function installHeapProbe() {
  const probe = { exports: undefined, memory: undefined };
  const OriginalInstance = WebAssembly.Instance;
  function Instance(module, imports) {
    const instance = new OriginalInstance(module, imports);
    // The first instance is the main thread's (workers instantiate in their own realms).
    if (!probe.exports && typeof instance.exports.malloc === 'function') {
      probe.exports = instance.exports;
    }
    return instance;
  }
  Instance.prototype = OriginalInstance.prototype;
  WebAssembly.Instance = Instance;

  const requested = process.env.ROLLDOWN_WASI_STRESS_INITIAL_PAGES;
  const initialPages =
    requested === 'min' ? readMemoryImport(readFileSync(threadedWasmPath())).minimum : requested;
  const OriginalMemory = WebAssembly.Memory;
  function Memory(descriptor) {
    const shared = descriptor?.shared === true;
    const memory = new OriginalMemory(
      shared && initialPages !== undefined
        ? { ...descriptor, initial: Number(initialPages) }
        : descriptor,
    );
    if (shared) probe.memory ??= memory;
    return memory;
  }
  Memory.prototype = OriginalMemory.prototype;
  WebAssembly.Memory = Memory;
  if (initialPages !== undefined) console.log(`loader initial memory: ${initialPages} pages`);
  return probe;
}

function heapSyncCounters(probe) {
  assert.equal(
    typeof probe.exports?.napi_wasm_heap_sync_stat,
    'function',
    'the threaded wasm exports napi_wasm_heap_sync_stat',
  );
  const stat = (index) => probe.exports.napi_wasm_heap_sync_stat(index);
  return {
    grows: stat(0),
    lockRefreshes: stat(1),
    lateRefreshes: stat(2),
    breakPages: stat(3),
    heapEndPages: stat(4),
    handoffRefreshes: stat(5),
    memoryPages: probe.memory ? probe.memory.buffer.byteLength / 65536 : -1,
  };
}

async function runCase(mode, fixture) {
  // A trap or a lost worker must end this process now: after a worker crash the exit
  // path can block, so print the error and kill ourselves.
  const die = (error) => {
    console.error(error?.stack ?? error);
    process.kill(process.pid, 'SIGKILL');
  };
  process.on('uncaughtException', die);
  process.on('unhandledRejection', die);

  const probe = installHeapProbe();
  const { rolldown } = await import('rolldown');
  const { getAsyncRuntimeConfig, getRuntimeCapabilities } = await import('rolldown/experimental');
  const { parse, transform } = await import('rolldown/utils');

  assert.equal(getRuntimeCapabilities().target, 'wasi-threads', 'needs the threaded WASI build');
  const config = getAsyncRuntimeConfig();
  if (process.env.ROLLDOWN_RUNTIME === 'single') {
    assert.equal(config.flavor, 'CurrentThread');
  } else {
    assert.equal(config.flavor, 'MultiThread');
    assert.equal(config.workerThreads, Number(process.env.ROLLDOWN_WORKER_THREADS ?? 2));
  }

  const holdMiB = Number(process.env.ROLLDOWN_WASI_STRESS_HOLD_MIB ?? 0);
  if (holdMiB > 0) {
    // Kept for the whole run: every later allocation sits above it. Node's `node:wasi`
    // rejects a pointer at or above 2^31, so the build below only passes if the heap still
    // has room under that line.
    assert.ok(probe.exports, 'captured the binding instance');
    const pointer = probe.exports.malloc(holdMiB * MIB) >>> 0;
    assert.notEqual(pointer, 0, `malloc(${holdMiB} MiB) failed`);
    console.log(
      `held ${holdMiB} MiB at 0x${pointer.toString(16)}-0x${(pointer + holdMiB * MIB).toString(16)}`,
    );
  }

  const files = readdirSync(fixture)
    .filter((file) => file.endsWith('.js'))
    .map((file) => {
      const id = path.join(fixture, file);
      return [id, readFileSync(id, 'utf8')];
    });
  assert.equal(files.length, MODULES);

  const plugin = {
    name: 'stress-async-hooks',
    async resolveId() {
      await new Promise((resolve) => setTimeout(resolve, 1));
    },
    async transform(code, id) {
      await new Promise((resolve) => setImmediate(resolve));
      return { code: `${code}\n/* ${path.basename(id)} ${'x'.repeat(2000)} */`, map: null };
    },
  };

  const build = async (plugins) => {
    const bundle = await rolldown({ input: path.join(fixture, 'm0.js'), plugins });
    try {
      const { output } = await bundle.generate({ format: 'esm' });
      assert.match(output[0].code, /function f0\(\)/);
    } finally {
      await bundle.close();
    }
  };
  const once = {
    bundle: () => build([plugin]),
    build: () => build([]),
    async parse() {
      const results = await Promise.all(files.map(([id, source]) => parse(id, source)));
      for (const result of results) assert.equal(result.errors.length, 0);
    },
    async transform() {
      const results = await Promise.all(
        files.map(([id, source]) => transform(id.replace(/\.js$/, '.ts'), source)),
      );
      for (const result of results) {
        assert.equal(result.errors.length, 0);
        assert.ok(result.code.length > 0);
      }
    },
  }[mode];
  assert.ok(once, `unknown stress mode ${mode}`);

  const rounds = { bundle: 2, build: 1 }[mode] ?? 3;
  for (let round = 0; round < rounds; round++) {
    await Promise.all(Array.from({ length: CONCURRENCY }, () => once()));
  }

  const counters = heapSyncCounters(probe);
  console.log(
    `HEAP_SYNC ${Object.entries(counters)
      .map(([key, value]) => `${key}=${value}`)
      .join(' ')}`,
  );
  assert.equal(counters.lateRefreshes, 0, 'a block ended past its thread refreshed size');
  const expectGrows = process.env.ROLLDOWN_WASI_STRESS_EXPECT_GROWS;
  if (expectGrows === 'zero') assert.equal(counters.grows, 0, 'expected no memory.grow');
  if (expectGrows === 'some') assert.ok(counters.grows > 0, 'expected the heap to grow');
  console.log(`STRESS_OK ${mode} ${config.flavor} workers=${config.workerThreads}`);
}
