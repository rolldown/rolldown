// Startup measurement for Cloudflare Workers: timed cold runs, first requests on the real
// entry, a CPU profile over the external inspector, and precise coverage from inside the
// isolate. Nothing here re-implements analysis the other modes have: the profile and
// coverage transforms are the shared ones, only the acquisition differs.

import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';

import { connect } from '../cdp.mjs';
import { median, quantile } from '../measure.mjs';
import { attributeStartupCoverage, profileStartup } from '../node/measure.mjs';
import {
  COVERAGE_FLAGS,
  COVERAGE_MIN_WRANGLER,
  EMPTY_NAME,
  LAB_COVERAGE_PATH,
  LAB_IMPORT_PATH,
  WRAPPER_NAME,
  createInstance,
  disposeQuietly,
  fetchText,
  timedImport,
  versionAtLeast,
} from './launch.mjs';

// The measured window excludes process boot, so the jitter is the host round trip to
// workerd: 1-2 ms on the floor runs seen so far. The floor is set above that, never below.
export const NOISE_FLOOR_MS = 3;
export const NOISE_FLOOR_PCT = 2;
export const COLD_MIN_BYTES = 4 * 1024;

const round1 = (v) => (v == null ? null : Math.round(v * 10) / 10);

/** N cold runs of one target (the main module, or the empty module for the floor). */
export async function gatherSeries(ctx, { target, runs, warmup = 1, onSample = null }) {
  for (let i = 0; i < warmup; i++) await timedImport(ctx, { target });
  const samples = [];
  for (let i = 0; i < runs; i++) {
    const run = await timedImport(ctx, { target });
    samples.push(run);
    onSample?.(i, run);
  }
  return samples;
}

/**
 * Fold the two series into the flat metric map. The floor is what the wrapper costs
 * when it imports an empty module: the host round trip, the proxy hop and the dynamic
 * import machinery. App startup is what is left, the part a bundle change can move.
 */
export function summarizeWorkerStartup({ floor, main }) {
  const floorMs = floor.map((s) => s.ms);
  const mainMs = main.map((s) => s.ms);
  const floorMedian = median(floorMs);
  const mainMedian = median(mainMs);
  const spread =
    mainMs.length > 1 && mainMedian > 0
      ? ((Math.max(...mainMs) - Math.min(...mainMs)) / mainMedian) * 100
      : 0;
  return {
    runs: main.length,
    metrics: {
      'worker.startup_ms': round1(mainMedian),
      'worker.startup_p75_ms': round1(quantile(mainMs, 0.75)),
      'worker.boot_floor_ms': round1(floorMedian),
      'worker.app_startup_ms': round1(Math.max(0, mainMedian - floorMedian)),
      'worker.instance_boot_ms': round1(median(main.map((s) => s.bootMs))),
    },
    guard: {
      allRunsCompleted: main.length > 0 && main.every((s) => typeof s.ms === 'number'),
      spreadPct: Math.round(spread * 10) / 10,
    },
    samples: { floor: floorMs.map(round1), main: mainMs.map(round1) },
  };
}

/** Silence after the last WebSocket message that ends a `ws:` request (waited, not counted). */
export const WS_QUIET_MS = 500;

/**
 * A pinned request: `/path` is a GET; `ws:/path` opens a WebSocket on the path, and
 * `ws:/path|<text>` also sends `<text>` once the socket is open. Agent chats and RPC run
 * over WebSockets, so a module deferred to the first RPC or chat turn only shows there.
 */
export function parseRequestSpec(spec) {
  const m = /^ws:(\/[^|]*)(?:\|([\s\S]*))?$/.exec(spec);
  return m ? { ws: true, path: m[1], message: m[2] ?? null } : { ws: false, path: spec, message: null };
}

/**
 * One WebSocket request, timed from the upgrade to the server's last message before
 * `WS_QUIET_MS` of silence: the reply to a sent message comes after any module load it
 * triggers. With no message and no server message, the time is the upgrade alone.
 */
export async function wsRequest(mf, p, message, { timeoutMs = 30_000 } = {}) {
  const t0 = performance.now();
  const res = await mf.dispatchFetch(`http://lab${p}`, { headers: { Upgrade: 'websocket' } });
  const ws = res.webSocket;
  if (res.status !== 101 || !ws) return { status: res.status, text: await res.text(), ms: performance.now() - t0 };
  ws.accept();
  let last = performance.now();
  let messages = 0;
  let bytes = 0;
  return new Promise((resolve) => {
    let quiet = null;
    const done = () => {
      clearTimeout(quiet);
      clearTimeout(hard);
      try {
        ws.close(1000);
      } catch {
        /* closed */
      }
      resolve({ status: 101, text: '', ms: last - t0, messages, bytes });
    };
    const hard = setTimeout(done, timeoutMs);
    const arm = () => {
      clearTimeout(quiet);
      quiet = setTimeout(done, WS_QUIET_MS);
    };
    ws.addEventListener('message', (event) => {
      messages++;
      bytes += typeof event.data === 'string' ? event.data.length : (event.data?.byteLength ?? 0);
      last = performance.now();
      arm();
    });
    ws.addEventListener('close', done);
    if (message != null) ws.send(message);
    arm();
  });
}

/**
 * First requests on a plain instance: the real entry, every export in place, startup
 * already done at boot. The first hit of a route that imports a deferred module pays
 * that load, which is the cost a deferral moved out of startup.
 */
export async function firstRequests(ctx, paths) {
  const mf = await createInstance(ctx, { plain: true });
  const rows = [];
  try {
    for (const p of paths) {
      const spec = parseRequestSpec(p);
      const t0 = performance.now();
      const res = spec.ws ? await wsRequest(mf, spec.path, spec.message) : await fetchText(mf, p);
      const ms = spec.ws ? res.ms : performance.now() - t0;
      rows.push({
        path: p,
        status: res.status,
        bytes: spec.ws && res.status === 101 ? res.bytes : res.text.length,
        ms: round1(ms),
        ...(spec.ws && res.status === 101 ? { messages: res.messages } : {}),
        error: res.status >= 500 ? res.text.slice(0, 200).replace(/\s+/g, ' ') : null,
      });
    }
  } finally {
    await disposeQuietly(mf);
  }
  return rows;
}

/**
 * CPU profile of the import, over the external inspector. workerd serves `Profiler.start`
 * and `stop` itself with its own CpuProfiler, so this route works on every workerd.
 */
export async function cpuProfileRun(ctx, { intervalUs = 100 } = {}) {
  const mf = await createInstance(ctx, { shared: { inspectorPort: 0 } });
  let cdp = null;
  try {
    const inspectorUrl = await mf.getInspectorURL();
    cdp = await connect(new URL('/core:user:lab', inspectorUrl).href);
    await cdp.send('Profiler.enable');
    await cdp.send('Profiler.setSamplingInterval', { interval: intervalUs });
    await cdp.send('Profiler.start');
    const imported = await fetchText(mf, LAB_IMPORT_PATH);
    const { profile } = await cdp.send('Profiler.stop');
    return {
      profile,
      importRan: imported.status === 200 && imported.text === 'ok',
      importText: imported.text,
    };
  } finally {
    try {
      cdp?.close();
    } catch {
      /* closed */
    }
    await disposeQuietly(mf);
  }
}

/** The summary `wrangler check startup` prints, so the two tools agree on their terms. */
export function summarizeProfile(profile) {
  const nodes = new Map(profile.nodes.map((n) => [n.id, n]));
  let sampled = 0;
  let idle = 0;
  let gc = 0;
  let program = 0;
  (profile.timeDeltas ?? []).forEach((d, i) => {
    sampled += d;
    const fn = nodes.get(profile.samples[i])?.callFrame.functionName;
    if (fn === '(idle)') idle += d;
    else if (fn === '(garbage collector)') gc += d;
    else if (fn === '(program)') program += d;
  });
  return {
    windowMs: round1((profile.endTime - profile.startTime) / 1000),
    sampledMs: round1(sampled / 1000),
    activeMs: round1((sampled - idle) / 1000),
    gcMs: round1(gc / 1000),
    programMs: round1(program / 1000),
    idleMs: round1(idle / 1000),
    samples: profile.samples.length,
  };
}

/**
 * Map a script URL onto the output dir. Miniflare reports `file://` paths on one version
 * and bare module names on another, and a profile recorded elsewhere carries another
 * directory: the longest path suffix that exists under the output dir wins.
 */
export function localFile(url, outDir) {
  if (!url || /^[a-z-]+:(?!\/\/)/.test(url)) return null;
  const parts = (url.startsWith('file://') ? new URL(url).pathname : url)
    .split('/')
    .filter(Boolean);
  for (let i = 0; i < parts.length; i++) {
    const candidate = path.join(outDir, parts.slice(i).join('/'));
    if (fs.existsSync(candidate)) return candidate;
  }
  return null;
}

/** `readChunk` for `profileStartup`: every chunk under the output dir that has a sourcemap. */
export function chunkResolver(outDir) {
  return (frameFile) => {
    const file = localFile(frameFile, outDir);
    if (!file || !fs.existsSync(`${file}.map`)) return null;
    return {
      code: fs.readFileSync(file, 'utf8'),
      map: JSON.parse(fs.readFileSync(`${file}.map`, 'utf8')),
    };
  };
}

/** Self time by source module through every chunk's sourcemap, plus the wrangler-style summary. */
export function attributeProfile(profile, { outDir, mainName }) {
  const summary = summarizeProfile(profile);
  const mapFile = path.join(outDir, `${mainName}.map`);
  if (!fs.existsSync(mapFile))
    return { summary, rows: null, totalMs: null, ownership: null, missingSourcemap: true };
  const code = fs.readFileSync(path.join(outDir, mainName), 'utf8');
  const map = JSON.parse(fs.readFileSync(mapFile, 'utf8'));
  const { rows, totalMs, ownership } = profileStartup(profile, {
    code,
    map,
    entryPath: path.join(outDir, mainName),
    readChunk: chunkResolver(outDir),
    baseUrl: pathToFileURL(`${outDir}/`),
  });
  return { summary, rows, totalMs, ownership, missingSourcemap: false };
}

/**
 * Precise coverage of the import, taken from INSIDE the isolate. The external inspector
 * cannot do it: workerd answers `Profiler.enable` itself and never forwards it to V8, so
 * `Profiler.startPreciseCoverage` fails with "Profiler is not enabled". The in-process
 * `node:inspector` session needs three compatibility flags, added for this run only, and a
 * workerd from 1.20260706.1 on. `tools` may point at a newer wrangler than the app's; the
 * timing and the profile keep the app's own workerd regardless.
 */
export async function coverageRun(ctx, { tools = null } = {}) {
  const used = tools ?? ctx.tools;
  const appFlags = ctx.workerOptions.compatibilityFlags ?? [];
  const addedFlags = COVERAGE_FLAGS.filter((f) => !appFlags.includes(f));
  const base = {
    source: null,
    result: null,
    addedFlags,
    workerd: used.versions.workerd,
    borrowed: Boolean(tools),
  };
  if (!versionAtLeast(used.versions.wrangler, COVERAGE_MIN_WRANGLER)) {
    return {
      ...base,
      unsupported: `wrangler ${used.versions.wrangler ?? '?'} (workerd ${used.versions.workerd ?? '?'}) predates the in-process inspector session; needs wrangler ${COVERAGE_MIN_WRANGLER} or later. Update wrangler, or pass --tools <dir> with a newer one for this run.`,
    };
  }
  const mf = await createInstance(ctx, { coverage: true, extraFlags: COVERAGE_FLAGS, tools: used });
  try {
    const { status, text } = await fetchText(mf, LAB_COVERAGE_PATH);
    if (status !== 200)
      return { ...base, unsupported: `coverage route answered ${status}: ${text.slice(0, 200)}` };
    const body = JSON.parse(text);
    if (body.unsupported)
      return {
        ...base,
        unsupported: `node:inspector session unavailable (${body.unsupported}) on workerd ${used.versions.workerd}`,
      };
    return { ...base, result: body.result, importError: body.error ?? null, unsupported: null };
  } finally {
    await disposeQuietly(mf);
  }
}

/** The package a sourcemap source belongs to, or `(app)`. */
export function packageOf(source) {
  const m = String(source).match(/node_modules\/((?:@[^/]+\/)?[^/]+)/g);
  return m ? m[m.length - 1].replace('node_modules/', '') : '(app)';
}

/**
 * Coverage attribution plus the views a real app needs: the 2% cold filter finds almost
 * nothing on real libraries, which run 5-30% of themselves at startup, so rank by bytes
 * parsed but not executed, and group by package.
 */
export function attributeCoverage(result, { outDir }) {
  const runtimeScripts = result
    .filter((s) =>
      /^(node|node-internal|cloudflare|cloudflare-internal|workerd):/.test(s.url ?? ''),
    )
    .map((s) => s.url);
  const local = [];
  for (const script of result) {
    const file = localFile(script.url, outDir);
    if (!file || file.endsWith(WRAPPER_NAME) || file.endsWith(EMPTY_NAME)) continue;
    local.push({ ...script, url: `file://${file}` });
  }
  const attributed = attributeStartupCoverage(local, { root: outDir });
  const modules = attributed.modules.map((m) => ({ ...m, idleBytes: m.totalBytes - m.readyBytes }));
  const totalBytes = modules.reduce((a, m) => a + m.totalBytes, 0);
  const readyBytes = modules.reduce((a, m) => a + m.readyBytes, 0);
  const byPackage = new Map();
  for (const m of modules) {
    const key = packageOf(m.source);
    const row = byPackage.get(key) ?? { pkg: key, totalBytes: 0, readyBytes: 0, modules: 0 };
    row.totalBytes += m.totalBytes;
    row.readyBytes += m.readyBytes;
    row.modules += 1;
    byPackage.set(key, row);
  }
  return {
    scripts: attributed.scripts,
    runtimeScripts,
    modules,
    totals: { totalBytes, readyBytes, moduleCount: modules.length },
    cold: modules
      .filter((m) => m.totalBytes >= COLD_MIN_BYTES && m.readyRatio <= 0.02)
      .sort((a, b) => b.totalBytes - a.totalBytes),
    idle: [...modules].sort((a, b) => b.idleBytes - a.idleBytes),
    packages: [...byPackage.values()].sort((a, b) => b.totalBytes - a.totalBytes),
  };
}
