// Concurrency stress for the threaded WASI artifact, under CurrentThread and under
// MultiThread with 4 workers.
//
// Before the `wasm_heap_sync` allocator, these loads trapped with "memory access out of
// bounds": V8 keeps a stale shared-memory size on threads that did not run memory.grow,
// and bounds-checks memory.fill / memory.copy against it. Without the fix, one run of this
// script fails nearly every time (CurrentThread parse 16x3 and MultiThread builds with a
// plugin trapped 10 of 10 runs each), so one passing run pins the fix.
// See internal-docs/wasi-shared-memory-grow/design.md
//
// Each case runs in its own child process: a wasm trap kills the process, and after a
// worker crash the process can hang on exit, so the parent enforces a per-case timeout.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const CHILD_FLAG = '--run-threaded-memory-stress-case';
const MODULES = 300;
const CONCURRENCY = 16;
const CASE_TIMEOUT_MS = Number(process.env.ROLLDOWN_WASI_STRESS_CASE_TIMEOUT_MS ?? 60_000);

const CURRENT_THREAD = { name: 'CurrentThread', env: {} };
const MULTI_THREAD = {
  name: 'MultiThread',
  env: { ROLLDOWN_RUNTIME: 'multi', ROLLDOWN_WORKER_THREADS: '4' },
};

// `bundle`: CONCURRENCY builds x 2 waves with an async JS plugin.
// `parse` / `transform`: CONCURRENCY concurrent passes over every module, 3 times.
const CASES = [
  { mode: 'bundle', runtime: CURRENT_THREAD },
  { mode: 'bundle', runtime: MULTI_THREAD },
  { mode: 'parse', runtime: CURRENT_THREAD },
  { mode: 'parse', runtime: MULTI_THREAD },
  { mode: 'transform', runtime: CURRENT_THREAD },
  { mode: 'transform', runtime: MULTI_THREAD },
];

const childIndex = process.argv.indexOf(CHILD_FLAG);
if (childIndex >= 0) {
  await runCase(process.argv[childIndex + 1], process.argv[childIndex + 2]);
} else {
  await runAll();
}

async function runAll() {
  const fixture = mkdtempSync(path.join(os.tmpdir(), 'rolldown-wasi-stress-'));
  const failures = [];
  const started = Date.now();
  try {
    writeChain(fixture, MODULES);
    for (const { mode, runtime } of CASES) {
      const label = `${mode} ${runtime.name}`;
      const result = await spawnCase(mode, fixture, runtime.env);
      const ok = result.code === 0 && !result.timedOut && result.output.includes('STRESS_OK');
      console.log(`${ok ? 'PASS' : 'FAIL'} ${label} (${result.ms} ms)`);
      if (!ok) {
        failures.push(label);
        const reason = result.timedOut
          ? `timed out after ${CASE_TIMEOUT_MS} ms`
          : `exited with ${result.signal ? `signal ${result.signal}` : `code ${result.code}`}`;
        console.log(`--- ${label}: ${reason}\n${result.output.trim()}\n---`);
      }
    }
  } finally {
    rmSync(fixture, { force: true, recursive: true });
  }
  console.log(`threaded WASI memory stress finished in ${Date.now() - started} ms`);
  if (failures.length > 0) {
    throw new Error(`threaded WASI memory stress failed: ${failures.join(', ')}`);
  }
}

function spawnCase(mode, fixture, runtimeEnv) {
  const env = { ...process.env, ...runtimeEnv };
  if (!('ROLLDOWN_RUNTIME' in runtimeEnv)) {
    delete env.ROLLDOWN_RUNTIME;
    delete env.ROLLDOWN_WORKER_THREADS;
  }
  const started = Date.now();
  const child = spawn(
    process.execPath,
    [fileURLToPath(import.meta.url), CHILD_FLAG, mode, fixture],
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

async function runCase(mode, fixture) {
  // A trap or a lost worker must end this process now: after a worker crash the exit
  // path can block, so print the error and kill ourselves.
  const die = (error) => {
    console.error(error?.stack ?? error);
    process.kill(process.pid, 'SIGKILL');
  };
  process.on('uncaughtException', die);
  process.on('unhandledRejection', die);

  const { rolldown } = await import('rolldown');
  const { getAsyncRuntimeConfig, getRuntimeCapabilities } = await import('rolldown/experimental');
  const { parse, transform } = await import('rolldown/utils');

  assert.equal(getRuntimeCapabilities().target, 'wasi-threads', 'needs the threaded WASI build');
  const config = getAsyncRuntimeConfig();
  if (process.env.ROLLDOWN_RUNTIME === 'multi') {
    assert.equal(config.flavor, 'MultiThread');
    assert.equal(config.workerThreads, Number(process.env.ROLLDOWN_WORKER_THREADS));
  } else {
    assert.equal(config.flavor, 'CurrentThread');
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

  const once = {
    async bundle() {
      const bundle = await rolldown({ input: path.join(fixture, 'm0.js'), plugins: [plugin] });
      try {
        const { output } = await bundle.generate({ format: 'esm' });
        assert.match(output[0].code, /function f0\(\)/);
      } finally {
        await bundle.close();
      }
    },
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

  const rounds = mode === 'bundle' ? 2 : 3;
  for (let round = 0; round < rounds; round++) {
    await Promise.all(Array.from({ length: CONCURRENCY }, () => once()));
  }
  console.log(`STRESS_OK ${mode} ${config.flavor} workers=${config.workerThreads}`);
}
