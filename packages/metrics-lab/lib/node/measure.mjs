// Startup measurement for server runtimes: timed spawn→ready runs, plus the two
// instrumented runs that say WHERE the time went (V8 precise coverage at the ready
// moment, and a CPU profile covering everything before it).
//
// Nothing here re-implements analysis that already exists. `coverageBySource` and
// `aggregateProfile` are the same pure V8-format transforms the browser side uses —
// Node emits the identical formats, so only the ACQUISITION differs.
//
// What deliberately does NOT carry over from the browser is the throttle model.
// `DEFAULT_THROTTLE` / net-scale calibration exist because page load is transfer
// bound; server startup does no network I/O and is disk+CPU bound. Carrying a
// throttle over (even pinned at x1) would invent a dimension the measurement does
// not have, so node mode has no throttle at all — and therefore no cross-scale
// comparability problem either.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { median, quantile } from '../measure.mjs';
import { coverageBySource } from '../coverage.mjs';
import { aggregateProfile } from '../profile.mjs';
import { measureBootFloor, startupRun } from './launch.mjs';

// A module this big that never executes before ready is worth a deferral look. Below
// it, the win is inside the noise of a single startup and the edit is not worth it.
export const COLD_MIN_BYTES = 4 * 1024;
// Startup is far quieter than a throttled page load (no network, no compositor), so
// the floor is tighter than the browser's 30ms — but never below timer/scheduler jitter.
export const NOISE_FLOOR_MS = 8;
export const NOISE_FLOOR_PCT = 2;

const round1 = (v) => (v == null ? null : Math.round(v * 10) / 10);

/**
 * N timed runs, no inspector — the numbers that get reported. `runOptsFor(index)` is
 * a factory rather than a fixed object so cold-cache mode can hand every run its own
 * empty compile-cache dir; warmup runs get index -1..-n so they never share one.
 */
export async function gatherStartupSamples(runOptsFor, { runs, warmup, onSample = null }) {
  for (let i = 0; i < warmup; i++) {
    process.stderr.write(`warmup ${i + 1}/${warmup}...\n`);
    await startupRun(runOptsFor(-1 - i));
  }
  const samples = [];
  for (let i = 0; i < runs; i++) {
    const run = await startupRun(runOptsFor(i));
    samples.push({ startupMs: run.elapsedMs, exitCode: run.exitCode });
    onSample?.(i, run);
  }
  return samples;
}

/**
 * Fold runs into the flat metric-id map. `bootFloorMs` is what an empty script costs
 * on this machine: subtracting it separates the cost the bundle owns from the runtime
 * boot it can never remove. Without that split a 340ms startup reads as 340ms of
 * addressable work when 30ms of it is physics.
 */
export function summarizeStartup(
  samples,
  { bootFloorMs = null, ready = null, resolutionMs = 0 } = {},
) {
  const values = samples.map((s) => s.startupMs).filter((v) => typeof v === 'number');
  const startup = median(values);
  const appMs =
    typeof startup === 'number' && typeof bootFloorMs === 'number'
      ? Math.max(0, startup - bootFloorMs)
      : null;
  return {
    runs: samples.length,
    metrics: {
      'runtime.startup_ms': round1(startup),
      'runtime.startup_p75_ms': round1(quantile(values, 0.75)),
      'runtime.boot_floor_ms': round1(bootFloorMs),
      // The addressable part: total minus the runtime's own boot.
      'runtime.app_startup_ms': round1(appMs),
    },
    guard: {
      allRunsCompleted: samples.length > 0 && samples.every((s) => typeof s.startupMs === 'number'),
      readySignal: ready,
      // A spread this wide means something other than the bundle moves run to run
      // (disk cache, a port retry, a background daemon) and deltas are not trustworthy.
      spreadPct:
        values.length > 1 && startup > 0
          ? Math.round(((Math.max(...values) - Math.min(...values)) / startup) * 1000) / 10
          : 0,
      // When the addressable time is only a few probe intervals wide, the digits after
      // the decimal point are quantization, not signal. Say so rather than letting an
      // agent chase a "+2ms regression" that is one poll tick.
      resolutionLimited:
        resolutionMs > 0 && typeof appMs === 'number' && appMs < 5 * resolutionMs
          ? { appMs: round1(appMs), resolutionMs }
          : null,
    },
    samples,
  };
}

// --- attribution -----------------------------------------------------------------

/** file:// script URLs that live under `root` and carry a sourcemap next to them. */
function localScript(url, root) {
  if (!url?.startsWith('file://')) return null;
  let file;
  try {
    file = fileURLToPath(url);
  } catch {
    return null;
  }
  const rel = path.relative(root, file);
  if (rel.startsWith('..') || path.isAbsolute(rel)) return null;
  const mapFile = `${file}.map`;
  if (!fs.existsSync(file) || !fs.existsSync(mapFile)) return null;
  return { file, rel: rel.replaceAll('\\', '/'), mapFile };
}

/**
 * One instrumented run: V8 precise coverage armed while the process is paused at
 * entry, snapshotted the moment it becomes ready. Every byte counted as executed
 * therefore HAD to run to get the server up.
 *
 * The known V8 blind spot applies here exactly as it does in the browser: a module's
 * top level counts as executed when it evaluates, so weight parked in top-level
 * literals looks "used". Function bodies are where the signal is clean.
 */
export async function startupCoverage(runOpts, { root }) {
  const run = await startupRun({
    ...runOpts,
    inspect: true,
    arm: async (cdp) => {
      await cdp.send('Profiler.enable');
      await cdp.send('Profiler.startPreciseCoverage', { callCount: false, detailed: true });
    },
    atReady: async (cdp) => cdp.send('Profiler.takePreciseCoverage'),
  });
  return attributeStartupCoverage(run.collected?.result ?? [], { root });
}

/**
 * Attribute V8 script coverage to source modules through each script's sourcemap.
 * Scripts outside `root` (node internals, native deps) are counted but not attributed —
 * they are real startup cost the bundler cannot address, and hiding them would make
 * the module list look like the whole story.
 */
export function attributeStartupCoverage(scriptCoverage, { root, readFile = null }) {
  const read = readFile ?? ((file) => fs.readFileSync(file, 'utf8'));
  const modules = [];
  const scripts = [];
  let external = 0;
  for (const script of scriptCoverage) {
    const local = localScript(script.url, root);
    if (!local) {
      if (script.url && !script.url.startsWith('node:')) external += 1;
      continue;
    }
    let code;
    let map;
    try {
      code = read(local.file);
      map = JSON.parse(read(local.mapFile));
    } catch {
      scripts.push({ file: local.rel, reason: 'unreadable-sourcemap' });
      continue;
    }
    const rows = coverageBySource({
      code,
      map,
      atPaint: script.functions ?? [],
      atSettle: [],
    });
    let totalBytes = 0;
    let evalBytes = 0;
    for (const [source, row] of rows.entries()) {
      modules.push({
        source,
        script: local.rel,
        totalBytes: row.totalBytes,
        // "paint" in the shared helper is just "the first snapshot"; here that
        // snapshot is taken at the ready moment.
        readyBytes: row.paintBytes,
        readyRatio: row.totalBytes ? row.paintBytes / row.totalBytes : 0,
      });
      totalBytes += row.totalBytes;
      evalBytes += row.paintBytes;
    }
    scripts.push({ file: local.rel, totalBytes, readyBytes: evalBytes });
  }
  modules.sort((a, b) => b.totalBytes - a.totalBytes);
  scripts.sort((a, b) => (b.totalBytes ?? 0) - (a.totalBytes ?? 0));
  return { modules, scripts, externalScriptCount: external };
}

/**
 * The names a coverage module might go by in the build's module graph, most specific
 * first. Coverage sources come from the sourcemap and are relative to the OUTPUT dir
 * ('../src/reporting.js'); graph ids are project-relative ('src/reporting.js'). Walking
 * the leading './' and '../' segments off is what lets the measured half and the static
 * half talk about the same module — without it every cold module prices as "no match"
 * and the scan quietly stops suggesting deferrals.
 */
export function graphIdCandidates(source) {
  const forms = [];
  let candidate = String(source).replaceAll('\\', '/');
  forms.push(candidate);
  while (/^\.\.?\//.test(candidate)) {
    candidate = candidate.replace(/^\.\.?\//, '');
    forms.push(candidate);
  }
  return forms;
}

/** Shipped into the startup path but never executed to get there — deferral candidates. */
export function coldModules(modules, { minBytes = COLD_MIN_BYTES } = {}) {
  return modules
    .filter((m) => m.totalBytes >= minBytes && m.readyRatio <= 0.02)
    .sort((a, b) => b.totalBytes - a.totalBytes);
}

/**
 * One instrumented run sampling from entry until ready: what RAN, and for how long.
 * This is the signal that can say "the bundle is not your problem" — if the self-time
 * sits in native addon loading or a TLS/DB handshake rather than in module evaluation,
 * no amount of deferral moves the number.
 */
export async function startupProfile(runOpts, { intervalUs = 200 } = {}) {
  const run = await startupRun({
    ...runOpts,
    inspect: true,
    arm: async (cdp) => {
      await cdp.send('Profiler.enable');
      await cdp.send('Profiler.setSamplingInterval', { interval: intervalUs });
      await cdp.send('Profiler.start');
    },
    atReady: async (cdp) => cdp.send('Profiler.stop'),
  });
  return run.collected?.profile ?? null;
}

/**
 * Split profile self-time into what the bundle owns and what it never will.
 * `appSources` is the entry sourcemap's source list — a bucket naming one of those
 * is app code; `(engine*)` is V8 itself; anything else is runtime/native frames.
 */
export function splitProfileOwnership(rows, appSources) {
  const app = new Set(appSources.map((s) => String(s).replaceAll('\\', '/')));
  let appMs = 0;
  let engineMs = 0;
  let runtimeMs = 0;
  for (const row of rows) {
    if (app.has(row.bucket)) appMs += row.ms;
    else if (row.bucket.startsWith('(engine')) engineMs += row.ms;
    else runtimeMs += row.ms;
  }
  const total = appMs + engineMs + runtimeMs;
  return {
    appMs: round1(appMs),
    engineMs: round1(engineMs),
    runtimeMs: round1(runtimeMs),
    // The three parts always sum to this. `aggregateProfile` drops sub-0.5ms buckets,
    // so its `totalMs` is slightly larger — reporting that as the headline next to a
    // split derived from the kept rows makes the numbers look like they fail to add up.
    accountedMs: round1(total),
    appShare: total > 0 ? Math.round((appMs / total) * 1000) / 10 : null,
  };
}

export function profileStartup(profile, { code, map, entryUrlSuffix }) {
  const { rows, totalMs } = aggregateProfile(profile, { code, map, entryUrlSuffix });
  return { rows, totalMs, ownership: splitProfileOwnership(rows, map.sources ?? []) };
}

export { measureBootFloor };
