// Server-runtime startup driver: spawn the runtime, wait for a DECLARED readiness
// signal, and — for instrumented runs — attach the V8 inspector before any user
// code evaluates.
//
// The browser hands us a readiness moment for free (FCP/LCP are observable from
// outside the page). A server process has no such universal signal: "up" means
// accepting connections for one app, a line on stdout for another, and clean exit
// for a CLI or a lambda-shaped entry. So readiness is part of the TARGET, declared
// once and pinned with it — the same role `expectedFeatures` plays for the demo app.
//
// Two run shapes, deliberately separate:
//   plain        — no inspector, no profiler. This is the number that gets reported;
//                  `--inspect-brk` alone shifts startup by tens of ms and would make
//                  every measurement a measurement of the harness.
//   instrumented — `--inspect-brk=0`, profiler/coverage armed while the process is
//                  still paused at entry, then released. Used for ATTRIBUTION only
//                  (which module cost what), never for the reported timing.

import { spawn } from 'node:child_process';
import net from 'node:net';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { connect } from '../cdp.mjs';

// Port/http readiness is discovered by polling, so a sample carries up to one interval
// of quantization. A local TCP connect attempt costs ~0.1ms, so the practical floor is
// setTimeout granularity rather than probe cost — poll as tightly as the timer allows.
// This matters more than it looks: on a target whose addressable startup is ~10ms, a
// 4ms quantum was 40% of the signal. `summarizeStartup` flags measurements that get
// close to this floor rather than reporting a precise-looking number it cannot support.
export const POLL_MS = 1;

const READY_MENU =
  '  --ready port:3000            a TCP listener is accepting (servers)\n' +
  "  --ready 'stdout:ready on'    a line matching this regex is printed\n" +
  '  --ready http://127.0.0.1:3000/health   an HTTP probe returns 2xx\n' +
  '  --ready exit                 the process exits 0 (CLIs, lambda-shaped entries)';

/**
 * Parse a `--ready` spec into a probe descriptor. Throws with the full menu — an
 * unparseable spec is the one error where guessing a default would silently
 * measure the wrong moment.
 */
export function parseReadySpec(spec) {
  const raw = String(spec ?? '').trim();
  if (raw === '' || raw === 'exit') return { kind: 'exit' };
  const port = raw.match(/^port:(\d{1,5})$/);
  if (port) {
    const value = Number(port[1]);
    if (value < 1 || value > 65535) throw new Error(`--ready port:${port[1]} is not a valid port.`);
    return { kind: 'port', port: value };
  }
  const stdout = raw.match(/^stdout:([\s\S]+)$/);
  if (stdout) {
    let pattern;
    try {
      pattern = new RegExp(stdout[1]);
    } catch (err) {
      throw new Error(
        `--ready stdout:<regex> — ${stdout[1]} is not a valid regex (${err.message}).`,
      );
    }
    return { kind: 'stdout', source: stdout[1], pattern };
  }
  if (/^https?:\/\//.test(raw)) return { kind: 'http', url: raw };
  throw new Error(`unrecognized --ready spec: ${raw}\nSupported forms:\n${READY_MENU}`);
}

export function describeReady(spec) {
  switch (spec.kind) {
    case 'port':
      return `port ${spec.port} accepting`;
    case 'stdout':
      return `stdout matches /${spec.source}/`;
    case 'http':
      return `${spec.url} returns 2xx`;
    default:
      return 'process exits 0';
  }
}

/** True once `port` accepts a connection. Never throws — a refused connect is "not yet". */
export function portAccepting(port, { host = '127.0.0.1', timeoutMs = 250 } = {}) {
  return new Promise((resolve) => {
    const socket = net.connect({ port, host });
    const done = (value) => {
      socket.destroy();
      resolve(value);
    };
    socket.setTimeout(timeoutMs, () => done(false));
    socket.once('connect', () => done(true));
    socket.once('error', () => done(false));
  });
}

/**
 * A port probe on an already-occupied port reports ready instantly and every run
 * measures nothing. Refuse before spawning rather than emitting a fast, wrong number.
 */
export async function assertReadyPortFree(spec) {
  if (spec.kind !== 'port') return;
  if (await portAccepting(spec.port, { timeoutMs: 150 })) {
    throw new Error(
      `port ${spec.port} is already accepting connections before the target started.\n` +
        'Every run would report ready immediately and measure nothing. Stop whatever holds\n' +
        `the port (lsof -i :${spec.port}), or point --ready at the port this build actually uses.`,
    );
  }
}

/** Has the declared readiness moment happened yet? `state` is owned by the runner. */
async function isReady(spec, state) {
  switch (spec.kind) {
    case 'exit':
      return state.exitCode === 0;
    case 'stdout':
      return spec.pattern.test(state.output);
    case 'port':
      return await portAccepting(spec.port);
    case 'http':
      try {
        const res = await fetch(spec.url, { signal: AbortSignal.timeout(1000) });
        return res.ok;
      } catch {
        return false;
      }
    default:
      return false;
  }
}

/**
 * Environment for one run. Two inherited variables would silently corrupt a
 * measurement and are always neutralized:
 *   NODE_V8_COVERAGE — makes V8 instrument every script, inflating startup.
 *   NODE_OPTIONS=--inspect* — a second inspector fights ours for the port and
 *                             breaks the "Debugger listening on" scrape.
 *
 * The compile cache is the server-side equivalent of the browser's service-worker
 * trap: with a warm cache, runs 2..N measure the cache rather than the bundle.
 *   cold (default) — a fresh cache dir per run: every run pays parse+compile, which
 *                    is what a cold start (new container, new lambda) actually pays.
 *   warm           — one shared dir across runs, primed by the warmup run.
 *   off            — leave the ambient environment alone, whatever the app does.
 */
export function runEnv({ cacheMode = 'cold', cacheDir = null, extraEnv = null } = {}) {
  const env = { ...process.env, ...extraEnv };
  delete env.NODE_V8_COVERAGE;
  if (env.NODE_OPTIONS) {
    const kept = env.NODE_OPTIONS.split(/\s+/).filter(
      (flag) => !/^--inspect(-brk|-port)?(=|$)/.test(flag),
    );
    if (kept.length) env.NODE_OPTIONS = kept.join(' ');
    else delete env.NODE_OPTIONS;
  }
  if (cacheMode === 'off') delete env.NODE_COMPILE_CACHE;
  else if (cacheDir) env.NODE_COMPILE_CACHE = cacheDir;
  return env;
}

export function freshCacheDir(stateDir, tag) {
  const dir = path.join(stateDir, 'compile-cache', `${tag}`);
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function killTree(child) {
  if (child.exitCode != null || child.signalCode != null) return Promise.resolve();
  return new Promise((resolve) => {
    const hard = setTimeout(() => {
      try {
        child.kill('SIGKILL');
      } catch {
        /* already gone */
      }
      resolve();
    }, 2000);
    child.once('exit', () => {
      clearTimeout(hard);
      resolve();
    });
    try {
      child.kill('SIGTERM');
    } catch {
      clearTimeout(hard);
      resolve();
    }
  });
}

/**
 * One spawn→ready run.
 *
 * `elapsedMs` is wall time from immediately-before-spawn to the first observation
 * of the readiness signal, so it includes runtime boot, module evaluation and
 * whatever the app does before declaring itself up — the whole cold path a
 * deployment pays. `inspector` (when requested) is handed to `arm` while the
 * process sits paused at entry, before any user code has evaluated.
 */
export async function startupRun({
  execPath = process.execPath,
  entry,
  args = [],
  execArgv = [],
  cwd,
  ready,
  env,
  timeoutMs = 60_000,
  inspect = false,
  arm = null,
  atReady = null,
}) {
  const state = { output: '', exitCode: null, signal: null };
  const argv = [...execArgv, ...(inspect ? ['--inspect-brk=0'] : []), entry, ...args];

  const started = process.hrtime.bigint();
  const child = spawn(execPath, argv, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
  const sinceStart = () => Number(process.hrtime.bigint() - started) / 1e6;

  let inspectorUrl = null;
  const collect = (chunk) => {
    state.output += chunk;
    if (state.output.length > 1_000_000) state.output = state.output.slice(-500_000);
    if (!inspectorUrl) {
      const match = state.output.match(/Debugger listening on (ws:\/\/\S+)/);
      if (match) inspectorUrl = match[1];
    }
  };
  child.stdout.setEncoding('utf8');
  child.stderr.setEncoding('utf8');
  child.stdout.on('data', collect);
  child.stderr.on('data', collect);
  child.once('exit', (code, signal) => {
    state.exitCode = code;
    state.signal = signal;
  });

  let cdp = null;
  try {
    if (inspect) {
      const deadline = Date.now() + 20_000;
      while (!inspectorUrl) {
        if (state.exitCode != null) {
          throw new Error(
            `the target exited (code ${state.exitCode}) before the inspector attached.\n${tail(state.output)}`,
          );
        }
        if (Date.now() > deadline) throw new Error('no "Debugger listening on" line within 20s.');
        await sleep(POLL_MS);
      }
      cdp = await connect(inspectorUrl);
      await cdp.send('Runtime.enable');
      if (arm) await arm(cdp);
      // Everything above happened while the process was paused at entry, so the
      // profiler sees module evaluation from its very first frame.
      await cdp.send('Runtime.runIfWaitingForDebugger');
    }

    const deadline = Date.now() + timeoutMs;
    let readyMs = null;
    for (;;) {
      if (await isReady(ready, state)) {
        readyMs = sinceStart();
        break;
      }
      // A non-zero exit can never become ready, whatever the probe is.
      if (state.exitCode != null && state.exitCode !== 0) {
        throw new Error(
          `the target exited with code ${state.exitCode} before becoming ready (${describeReady(ready)}).\n${tail(state.output)}`,
        );
      }
      if (state.exitCode != null && ready.kind !== 'exit') {
        throw new Error(
          `the target exited cleanly but never signalled ${describeReady(ready)}.\n` +
            `If this entry is meant to finish rather than stay up, measure it with --ready exit.\n${tail(state.output)}`,
        );
      }
      if (Date.now() > deadline) {
        throw new Error(readyTimeoutMessage(ready, timeoutMs, state));
      }
      await sleep(POLL_MS);
    }

    const collected = atReady && cdp ? await atReady(cdp) : null;
    return { elapsedMs: readyMs, output: state.output, exitCode: state.exitCode, collected };
  } finally {
    try {
      cdp?.close();
    } catch {
      /* already closed */
    }
    await killTree(child);
  }
}

/**
 * Every readiness timeout is a target-configuration mistake, so the message is the
 * menu of correct specs rather than a bare "timed out" — an agent that reads only
 * the last line still learns the fix.
 */
function readyTimeoutMessage(ready, timeoutMs, state) {
  const stillRunning = state.exitCode == null;
  if (ready.kind === 'exit' && stillRunning) {
    return (
      `the target was still running after ${Math.round(timeoutMs / 1000)}s and --ready defaulted to "exit".\n` +
      'This looks like a long-lived server. Declare how it signals readiness:\n' +
      `${READY_MENU}\n${tail(state.output)}`
    );
  }
  return (
    `the target never signalled ${describeReady(ready)} within ${Math.round(timeoutMs / 1000)}s.\n` +
    `Check the spec matches what this build prints or listens on:\n${READY_MENU}\n${tail(state.output)}`
  );
}

function tail(output, lines = 12) {
  const text = output.trimEnd();
  if (!text) return '(the target produced no output)';
  return `--- last output ---\n${text.split('\n').slice(-lines).join('\n')}`;
}

/**
 * The floor: what an empty script costs on this machine with this runtime. Startup
 * work the bundle can never remove (binary load, V8 init, bootstrap) — without it,
 * "340ms" reads as 340ms of addressable cost when 30ms of it is physics.
 */
export async function measureBootFloor({ execPath = process.execPath, runs = 5, env } = {}) {
  const probe = path.join(os.tmpdir(), `metrics-lab-boot-${process.pid}.mjs`);
  fs.writeFileSync(probe, 'process.exit(0)\n');
  try {
    const samples = [];
    for (let i = 0; i < runs; i++) {
      const run = await startupRun({
        execPath,
        entry: probe,
        ready: { kind: 'exit' },
        env: env ?? runEnv({ cacheMode: 'off' }),
        timeoutMs: 30_000,
      });
      samples.push(run.elapsedMs);
    }
    return samples;
  } finally {
    fs.rmSync(probe, { force: true });
  }
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
