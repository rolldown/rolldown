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
import { sourceLocator } from '../profile.mjs';
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
// Under the floor, pairs that fall on one side still decide: a no-op lands on both sides.
// The sign test allows a no-op that chance at most (5 of 5 pairs, or 8 of 9). The median
// must also be this large, so rounding alone cannot make the series one-sided.
export const PAIRED_MAX_TAIL = 1 / 32;
export const PAIRED_MIN_MS = 0.5;
// A 5-pair series with 4 pairs on one side is ambiguous: `worker-scan` times more pairs.
export const PAIRED_EXTEND_TO = 9;
// Beyond the floor, the median decides only when the pairs agree: at most 1 of 5 pairs (2 of
// 9) on the other side. A median beyond the floor with pairs on both sides is a few slow runs.
export const PAIRED_BEYOND_TAIL = 1 / 4;

/** The chance that a no-op puts at least `k` of `n` pairs on one given side. */
export function signTail(k, n) {
  let ways = 0;
  let choose = 1; // C(n, i), built up from C(n, 0)
  for (let i = 0; i <= n; i++) {
    if (i >= k) ways += choose;
    choose = (choose * (n - i)) / (i + 1);
  }
  return ways / 2 ** n;
}

const sideCount = (pairs, deltaMs) => pairs.filter((p) => (deltaMs < 0 ? p < 0 : p > 0)).length;

/**
 * Verdict on the pairs `current - pinned` timed in one scan. Beyond the floor the median
 * decides when the pairs agree (`mixed` when they do not); under it, a change that the sign
 * test puts on one side still counts, as `small`.
 */
export function pairedVerdict({ pairs, deltaMs, appStartupMs }) {
  const floor = Math.max(NOISE_FLOOR_MS, (NOISE_FLOOR_PCT / 100) * (appStartupMs ?? 0));
  const kind = deltaMs < 0 ? 'improvement' : 'regression';
  const side = sideCount(pairs, deltaMs);
  if (Math.abs(deltaMs) >= floor) {
    const agree = pairs.length < 3 || signTail(side, pairs.length) <= PAIRED_BEYOND_TAIL;
    return agree
      ? { kind, small: false, floor, side }
      : { kind: 'noise', small: false, floor, side, mixed: deltaMs < 0 ? 'lower' : 'higher' };
  }
  const oneSided =
    Math.abs(deltaMs) >= PAIRED_MIN_MS && signTail(side, pairs.length) <= PAIRED_MAX_TAIL;
  return oneSided ? { kind, small: true, floor, side } : { kind: 'noise', small: false, floor };
}

/** True when a short series leans one way, or disagrees beyond the floor: time more pairs. */
export function needsMorePairs({ pairs, deltaMs, appStartupMs }) {
  if (pairs.length >= PAIRED_EXTEND_TO) return false;
  const verdict = pairedVerdict({ pairs, deltaMs, appStartupMs });
  if (verdict.kind !== 'noise') return false;
  if (verdict.mixed) return true;
  return Math.abs(deltaMs) >= PAIRED_MIN_MS && sideCount(pairs, deltaMs) >= pairs.length - 1;
}

/** The words `worker-scan` prints for a `pairedVerdict`. */
export function pairedCall(verdict, pairCount) {
  const floor = Math.round(verdict.floor);
  if (verdict.mixed) {
    return `within noise (floor ${floor}ms) - the pairs disagree: the median is beyond the floor, ${verdict.mixed} in only ${verdict.side} of ${pairCount} pairs`;
  }
  if (verdict.kind === 'noise') return `within noise (floor ${floor}ms) - no effect proven`;
  const word = verdict.kind === 'improvement' ? 'IMPROVEMENT' : 'REGRESSION';
  if (!verdict.small) return `${word} beyond noise`;
  const side = verdict.kind === 'improvement' ? 'lower' : 'higher';
  const count = verdict.side === pairCount ? `all ${pairCount}` : `${verdict.side} of ${pairCount}`;
  return `${word} beyond noise - small: ${side} in ${count} pairs (under the ${floor}ms floor)`;
}

// Startup that lost this much against the pinned build is smaller, whatever the timing says:
// a share of the bytes, or of the modules (a text or data module counts few bytes in the
// graph and still loads on its own).
export const CLOSURE_MIN_BYTES = 32 * 1024;
export const CLOSURE_MIN_BYTE_SHARE = 0.01;
export const CLOSURE_MIN_MODULES = 50;
export const CLOSURE_MIN_MODULE_SHARE = 0.1;

/**
 * The call the modules at startup add when the paired timing proves nothing. `closure`: the
 * static closure diff against the pinned build (`left`, `entered`, `leftBytes`,
 * `enteredBytes`, `stillCount` and `stillBytes` for the modules that left but a startup chunk
 * still loads, `pinnedBytes`, `pinnedModules`). `loaded`: the delta of
 * `worker.startup_loaded_bytes`. Returns `{ kind: 'keep', bytes, modules }` when startup lost
 * enough, `{ kind: 'none' }` when neither moved, else null.
 */
export function closureCall(closure, loaded, verdict) {
  if (verdict?.kind !== 'noise' || (!closure && !loaded)) return null;
  const moved = Boolean(closure && (closure.left.length || closure.entered.length));
  const loadedMoved = Boolean(loaded && Math.abs(loaded.delta) >= 1024);
  const staticBytes = closure ? closure.leftBytes - closure.stillBytes - closure.enteredBytes : 0;
  const bytes = Math.max(staticBytes, loaded ? -loaded.delta : 0);
  const before = Math.max(closure?.pinnedBytes ?? 0, loaded?.prev ?? 0);
  const modules = closure ? closure.left.length - closure.stillCount - closure.entered.length : 0;
  const byBytes = bytes >= Math.max(CLOSURE_MIN_BYTES, CLOSURE_MIN_BYTE_SHARE * before);
  const byModules =
    modules >=
    Math.max(CLOSURE_MIN_MODULES, CLOSURE_MIN_MODULE_SHARE * (closure?.pinnedModules ?? 0));
  if (byBytes || byModules) return { kind: 'keep', bytes: byBytes ? bytes : null, modules };
  return moved || loadedMoved ? null : { kind: 'none' };
}

// V8 sizes the young generation by what the isolate allocated so far. A deferral that takes a
// large early allocation out of startup can keep it small for longer, and the extra scavenges
// can cost more than the code saved. When startup only lost modules and the import still got
// slower, `worker-scan` times both builds again with these flags, which fix that size.
export const FIXED_YOUNG_V8_FLAGS = ['--min-semi-space-size=16'];

/** True when the static closure only lost modules against the pinned build. */
export function closureOnlyShrank(closure) {
  return Boolean(closure && closure.left.length > 0 && closure.entered.length === 0);
}

/**
 * The verdict the keep rule uses. A regression that the fixed-young-generation series does
 * not repeat is V8 heap sizing, not the change: the keep rule reads that series instead.
 */
export function keepVerdict(paired, fixedYoung) {
  const verdict = pairedVerdict(paired);
  if (verdict.kind !== 'regression' || !fixedYoung) return verdict;
  const fixed = pairedVerdict(fixedYoung);
  return fixed.kind === 'regression' ? verdict : { ...fixed, kind: 'noise', heapSizing: true };
}

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
  return m
    ? { ws: true, path: m[1], message: m[2] ?? null }
    : { ws: false, path: spec, message: null };
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
  if (res.status !== 101 || !ws)
    return { status: res.status, text: await res.text(), ms: performance.now() - t0 };
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

/**
 * `fileOf(url)`: the output file of a frame, relative to the output dir, or null.
 * `locate(file, line, column)`: the source module at a position of that file, from its sourcemap.
 */
export function outputLocator(outDir) {
  const readChunk = chunkResolver(outDir);
  const locators = new Map();
  const fileOf = (url) => {
    const file = localFile(url, outDir);
    return file ? path.relative(outDir, file).replaceAll('\\', '/') : null;
  };
  const locate = (file, line, column) => {
    if (!locators.has(file)) {
      const chunk = readChunk(file);
      locators.set(file, chunk ? sourceLocator(chunk.code, chunk.map) : null);
    }
    const source = locators.get(file)?.locate(line, column);
    return source && source !== '(unmapped)' ? source : null;
  };
  return { fileOf, locate };
}

/** Self time by source module through every chunk's sourcemap, plus the wrangler-style summary. */
export function attributeProfile(profile, { outDir, mainName }) {
  const summary = summarizeProfile(profile);
  const mapFile = path.join(outDir, `${mainName}.map`);
  // An entry that only re-exports from a manual chunk has no map; the chunks that run do.
  const map = fs.existsSync(mapFile) ? JSON.parse(fs.readFileSync(mapFile, 'utf8')) : null;
  const anyMap = () =>
    fs.readdirSync(outDir, { recursive: true }).some((f) => String(f).endsWith('.js.map'));
  if (!map && !anyMap())
    return { summary, rows: null, totalMs: null, ownership: null, missingSourcemap: true };
  const code = fs.readFileSync(path.join(outDir, mainName), 'utf8');
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
