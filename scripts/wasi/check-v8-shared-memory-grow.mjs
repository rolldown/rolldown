// Probe whether this Node's V8 still keeps a stale shared-memory size per thread
// after another thread runs `memory.grow`. The threaded WASI binding works
// around that in `crates/rolldown_binding/src/wasm_heap_sync.rs`; see
// internal-docs/wasi-shared-memory-grow/design.md ("When to remove").
//
// Two workers share one wasm memory. Worker B enters one long wasm activation
// that never allocates and never grows. Worker A grows the memory by one page
// per round and hands B a pointer into the new page through a mailbox word in
// that memory. B then runs memory.copy, memory.fill and i32.atomic.rmw.add on it.
//   case 1: B does no refresh      -> a trap here means the V8 bug is present
//   case 2: B runs memory.grow(0)  -> the workaround; expected to pass
// Every run is a fresh process, so B's activation is its first call ("cold":
// baseline Liftoff code under Node's default dynamic tiering), which is the
// deterministic shape. `--activation=warm` tiers B up to TurboFan first (a
// race: a few runs in 20 trap on hosts that have the bug).
//
// Usage: node scripts/wasi/check-v8-shared-memory-grow.mjs
//          [--runs=20] [--rounds=200] [--timeout=30000] [--activation=cold|warm]
//          [--expect-present | --expect-absent]
// Node flags given to this script (e.g. --liftoff-only) are passed to every run.
// Exits 0 whatever the verdict, unless --expect-* is given and does not match;
// exits 1 when a run fails to set up or times out and case 1 never trapped.
//
// Not a CI gate: it documents a host property. The wasm module is built from
// check-v8-shared-memory-grow.wat (rebuild steps in its header).

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { Worker, isMainThread, parentPort, workerData } from 'node:worker_threads';

// spellchecker:off (base64 of the wasm module)
const HANDOFF_WASM_BASE64 =
  'AGFzbQEAAAABDgJgAX8Bf2AEf39/fwF/AhIBA2VudgZtZW1vcnkCAwGAgAQDAwIAAQccAglncm93' +
  'X2xvb3AAAAxoYW5kb2ZmX2xvb3AAAQr8AgKTAQEDf0HAAEGRosSIATYCAEHEAEGixIiRAjYCAEHI' +
  'AEGz5syZAzYCAEHMAEHEiJGiBDYCAAJAA0AgASAATw0BIAFBAWohAUEBQAAhAyADQX9GBEBBfw8L' +
  'IANBgIAEbEGAAWohAiACIAE2AgBBACAC/hcCAANAQRj+EAIABEBBfg8LQQT+EAIAIAFHDQALDAAL' +
  'CyABC+QBAQR/IANFBEBBEEEB/hcCAAsCQANAIAQgAE8NASAEQQFqIQQgAwRAQYAIIQUFA0BBAP4Q' +
  'AgAhBSAFIAZGDQALIAUhBkEMIAT+FwIACyABQQFGBEBBJEEAQAD+FwIACyABQQJGBEBBJD8A/hcC' +
  'AAsgAkEBcQRAQQhBAf4XAgAgBUHAAEEQ/AoAAAsgAkECcQRAQQhBAv4XAgAgBUHaAEEQ/AsACyAC' +
  'QQRxBEBBCEED/hcCACAHIAVBAf4eAgBqIQcLQQhBAP4XAgAgA0UEQEEEIAT+FwIACwwACwtBICAH' +
  'NgIAIAQL';
// spellchecker:on

// handoff_loop $mode: 0 no refresh, 1 memory.grow(0). $ops 7 = copy + fill + atomic rmw.
const CASES = [
  { id: 1, label: 'no refresh', mode: 0 },
  { id: 2, label: 'memory.grow(0)', mode: 1 },
];
const OPS_ALL = 7;
const STAGES = ['idle', 'memory.copy', 'memory.fill', 'i32.atomic.rmw.add'];
// i32 word indexes of the page-0 layout in the .wat header.
const WORD_STAGE = 2;
const WORD_ROUND = 3;
const WORD_B_ENTERED = 4;
const WORD_ABORT = 6;

if (!isMainThread) {
  runWorker();
} else if (process.argv[2] === '--child') {
  runChild(Number(process.argv[3]), Number(process.argv[4]), process.argv[5] === 'warm');
} else {
  runParent();
}

function runParent() {
  const opts = { runs: 20, rounds: 200, timeout: 30_000, activation: 'cold', expect: null };
  for (const arg of process.argv.slice(2)) {
    const [key, value] = arg.split('=');
    if (key === '--runs') opts.runs = Number(value);
    else if (key === '--rounds') opts.rounds = Number(value);
    else if (key === '--timeout') opts.timeout = Number(value);
    else if (key === '--activation') opts.activation = value;
    else if (key === '--expect-present') opts.expect = 'PRESENT';
    else if (key === '--expect-absent') opts.expect = 'ABSENT';
    else usage(`unknown argument: ${arg}`);
  }
  for (const key of ['runs', 'rounds', 'timeout']) {
    if (!Number.isInteger(opts[key]) || opts[key] < 1) usage(`--${key} must be a positive integer`);
  }
  if (!['cold', 'warm'].includes(opts.activation)) usage('--activation must be cold or warm');

  const flags = process.execArgv.join(' ') || '(none)';
  console.log(
    `node ${process.version}, V8 ${process.versions.v8}, ${process.platform}-${process.arch}, ` +
      `flags ${flags}, ${opts.activation} activation, ${opts.runs} runs x ${opts.rounds} rounds per case`,
  );

  const script = fileURLToPath(import.meta.url);
  const summary = {};
  for (const kase of CASES) {
    const tally = { trap: 0, pass: 0, error: 0, rounds: [], stages: new Set(), errors: [] };
    for (let run = 0; run < opts.runs; run++) {
      const child = spawnSync(
        process.execPath,
        [
          ...process.execArgv,
          script,
          '--child',
          String(kase.mode),
          String(opts.rounds),
          opts.activation,
        ],
        { encoding: 'utf8', timeout: opts.timeout, killSignal: 'SIGKILL' },
      );
      const line = child.stdout?.split('\n').find((l) => l.startsWith('RESULT '));
      const result = line ? JSON.parse(line.slice('RESULT '.length)) : null;
      if (result?.status === 'trap') {
        tally.trap++;
        tally.rounds.push(result.round);
        tally.stages.add(STAGES[result.stage] ?? `stage ${result.stage}`);
      } else if (result?.status === 'pass') {
        tally.pass++;
      } else {
        tally.error++;
        const why =
          child.error?.code === 'ETIMEDOUT'
            ? `timed out after ${opts.timeout} ms`
            : (result?.message ??
              (child.stderr.trim().split('\n').pop() || `exit ${child.status}`));
        tally.errors.push(why);
      }
    }
    summary[kase.id] = tally;
    let detail = '';
    if (tally.trap > 0) {
      detail = `, rounds ${Math.min(...tally.rounds)}-${Math.max(...tally.rounds)}, at ${[...tally.stages].join(' / ')}`;
    }
    if (tally.error > 0)
      detail += `, ${tally.error} errors (${[...new Set(tally.errors)].join('; ')})`;
    console.log(`case ${kase.id} (${kase.label}): ${tally.trap}/${opts.runs} trapped${detail}`);
  }

  const [one, two] = [summary[1], summary[2]];
  let verdict;
  if (one.trap > 0) verdict = 'PRESENT';
  else if (one.error === 0) verdict = 'ABSENT';
  else verdict = 'INCONCLUSIVE';
  if (two.trap > 0) {
    console.log('warning: case 2 trapped, so memory.grow(0) did not refresh the size on this host');
  }
  console.log(
    `V8 stale shared-memory size: ${verdict} (case1 ${one.trap}/${opts.runs} trapped, case2 ${two.trap}/${opts.runs})`,
  );
  if (verdict === 'INCONCLUSIVE') process.exit(1);
  if (opts.expect && opts.expect !== verdict) {
    console.log(`expected ${opts.expect}`);
    process.exit(1);
  }
}

function usage(message) {
  console.error(message);
  console.error(
    'usage: node scripts/wasi/check-v8-shared-memory-grow.mjs [--runs=20] [--rounds=200] ' +
      '[--timeout=30000] [--activation=cold|warm] [--expect-present | --expect-absent]',
  );
  process.exit(1);
}

// One run: worker B (reader) first, then worker A (grower) once B is ready.
// Prints one `RESULT {json}` line and exits.
function runChild(mode, rounds, warm) {
  const memory = new WebAssembly.Memory({ initial: 1, maximum: 65_536, shared: true });
  const module = new WebAssembly.Module(Buffer.from(HANDOFF_WASM_BASE64, 'base64'));
  const base = { memory, module, rounds, mode, warm };
  let done = false;
  const finish = (result) => {
    if (done) return;
    done = true;
    console.log(`RESULT ${JSON.stringify(result)}`);
    // The workers may still be spinning in wasm; do not wait for them.
    process.exit(0);
  };
  const url = new URL(import.meta.url);
  const reader = new Worker(url, { workerData: { ...base, role: 'reader' } });
  reader.on('error', (e) => finish({ status: 'error', message: `reader: ${e.message}` }));
  reader.on('message', (m) => {
    if (m.type === 'ready') {
      const grower = new Worker(url, { workerData: { ...base, role: 'grower' } });
      grower.on('error', (e) => finish({ status: 'error', message: `grower: ${e.message}` }));
      grower.on('message', (g) => finish({ status: 'error', message: `grower: ${g.message}` }));
    } else {
      finish(m);
    }
  });
}

function runWorker() {
  const { memory, module, role, rounds, mode, warm } = workerData;
  const { exports } = new WebAssembly.Instance(module, { env: { memory } });
  const words = () => new Int32Array(memory.buffer);
  if (role === 'grower') {
    // Wait until B is inside its long activation (B sets the word from wasm).
    while (Atomics.load(words(), WORD_B_ENTERED) !== 1) Atomics.wait(words(), WORD_B_ENTERED, 0, 5);
    if (exports.grow_loop(rounds) === -1) parentPort.postMessage({ message: 'memory.grow failed' });
    return;
  }
  const handoffLoop = exports.handoff_loop;
  if (warm) {
    // Same function on page 0 only, until V8 tiers it up; wasm has no OSR, so
    // the next call runs the tiered code. Then let the background compile land.
    for (let i = 0; i < 20; i++) handoffLoop(100_000, mode, OPS_ALL, 1);
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 200);
    for (let i = 0; i < 5; i++) handoffLoop(100_000, mode, OPS_ALL, 1);
  }
  parentPort.postMessage({ type: 'ready' });
  try {
    handoffLoop(rounds, mode, OPS_ALL, 0);
    parentPort.postMessage({ status: 'pass' });
  } catch (e) {
    const v = words();
    Atomics.store(v, WORD_ABORT, 1); // let A leave its wait loop
    if (!(e instanceof WebAssembly.RuntimeError) || !e.message.includes('out of bounds')) {
      parentPort.postMessage({ status: 'error', message: `reader: ${e.message}` });
      return;
    }
    parentPort.postMessage({
      status: 'trap',
      round: Atomics.load(v, WORD_ROUND),
      stage: Atomics.load(v, WORD_STAGE),
      message: e.message,
    });
  }
}
