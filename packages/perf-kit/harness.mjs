#!/usr/bin/env node
// Browser-loading perf harness — the measurement + diagnosis primitives an agent
// drives to run the optimize loop (see README.md / AGENTS.md).
// Prototype of metrics-plan Phase 3b (lab runner) + 3c (coverage).
//
// The short version:
//   node harness.mjs scan --app <appDir>     everything in one go: N timed runs +
//                                            coverage + boot profile + verdict.
//                                            The target is remembered; afterwards
//                                            plain `node harness.mjs scan` works.
//   node harness.mjs verdict                 the only "done" that counts.
//
// All commands work from any working directory.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { spawnSync } from 'node:child_process';

import { launchBrowser } from './lib/cdp.mjs';
import { startServer } from './lib/serve.mjs';
import { buildApp } from './lib/build.mjs';
import { installCheckoutRolldown, reportSupportNote } from './lib/rolldown-support.mjs';
import {
  FEATURES,
  FEATURE_NAMES,
  featureModes,
  generateApp,
  setFeatureMode,
} from './lib/gen-app.mjs';
import {
  DEFAULT_THROTTLE,
  deltaSection,
  lcpElementOf,
  median,
  parseNetworkFlags,
  scaleThrottle,
  summarize,
  throttleFromProfile,
  throttleId,
  timedRun,
} from './lib/measure.mjs';
import {
  COLD_OPEN_MIN_BYTES,
  attributeChunks,
  coldAtPaintModules,
  coverageRun,
  largeAtPaintModules,
  siblingVariantGroups,
} from './lib/coverage.mjs';
import { aggregateProfile, profileEvidence, profileRun } from './lib/profile.mjs';
import { analyzeChunkDiscovery, loadChunkGraph } from './lib/chunk-discovery.mjs';
import {
  chunkDiscoveryAdvice,
  comparePaints,
  renderGapEvidence,
} from './lib/paint-diagnostics.mjs';
import {
  deferAllInto,
  evalOverrides,
  loadModuleGraph,
  moduleGraphCandidates,
  reportDirFor,
  reportDistMismatch,
  resolveModule,
  whatIf,
  withRoots,
} from './lib/module-graph.mjs';
import { minCut } from './lib/min-cut.mjs';
import { diffGraphs } from './lib/graph-diff.mjs';
import {
  POLL_MS,
  assertReadyPortFree,
  describeReady,
  freshCacheDir,
  parseReadySpec,
  runEnv,
} from './lib/node/launch.mjs';
import {
  NOISE_FLOOR_MS as NODE_NOISE_FLOOR_MS,
  NOISE_FLOOR_PCT as NODE_NOISE_FLOOR_PCT,
  coldModules,
  gatherStartupSamples,
  graphIdCandidates,
  measureBootFloor,
  profileStartup,
  startupCoverage,
  startupProfile,
  summarizeStartup,
} from './lib/node/measure.mjs';
import {
  COVERAGE_MIN_WRANGLER,
  EMPTY_NAME,
  classifyModules,
  createStderrCollector,
  loadWorkerOptions,
  resolveWorkerTools,
  timedImport,
} from './lib/worker/launch.mjs';
import {
  NOISE_FLOOR_MS as WORKER_NOISE_FLOOR_MS,
  NOISE_FLOOR_PCT as WORKER_NOISE_FLOOR_PCT,
  attributeCoverage,
  attributeProfile,
  coverageRun as workerCoverageRun,
  cpuProfileRun,
  firstRequests,
  gatherSeries,
  outputLocator,
  summarizeWorkerStartup,
} from './lib/worker/measure.mjs';
import { evaluatingModules, shortModule, topLevelCalls, whyModule } from './lib/worker/attrib.mjs';
import { ATTRIB_DIR, SHIM_FILE, attribOutDir, writeViteConfigShim } from './lib/worker/config.mjs';
import {
  discoverPages,
  gitState,
  hashDist,
  normalizePagePath,
  parityPaths,
  saveAnchor,
} from './lib/parity/anchor.mjs';
import { compareLcp, describeLcp, lcpReason } from './lib/parity/compare.mjs';
import { runParity, pageSlug } from './lib/parity/run.mjs';
import { cmdField } from './lib/field/run.mjs';
import { onInterrupt } from './lib/interrupt.mjs';
import { cmdRecord, cmdReplay } from './lib/replay/run.mjs';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.join(ROOT, 'app');
// State placement: inside the package while developing it from the repo, but for
// an installed copy (anywhere under node_modules) state belongs to the USER'S
// project — node_modules is wiped on reinstall and should never hold results.
const STATE_DIR =
  process.env.ROLLDOWN_PERF_KIT_STATE ??
  (/[\\/]node_modules[\\/]/.test(ROOT)
    ? path.join(process.cwd(), '.perf-kit')
    : path.join(ROOT, 'state'));
const TARGET_FILE = path.join(STATE_DIR, 'target.json');
// The demo build's report: report mode writes one dir per output dir, under the build cwd.
const BUILD_METRICS_DIR = reportDirFor(APP_DIR, 'dist');
const CHROME_PROFILE_DIR = path.join(STATE_DIR, 'chrome-profile');

// Decision thresholds the runbook references. The harness only REPORTS against
// them; accepting or reverting a change is the loop driver's (agent's) call.
const NOISE_FLOOR_MS = 30;
const NOISE_FLOOR_PCT = 2;
// A large first-run LCP change can end a probe early. A keep/revert decision
// still needs a full scan of both paint metrics with --no-early-stop.
// Keep the throttle fixed: changing it can invert a comparison.
const EARLY_STOP_FACTOR = 5;
// Per-target throttle calibration: the ONE throttle dimension that may vary is
// network bandwidth, in named discrete steps, chosen once per target and PINNED
// (it is part of the measurement's identity - agents, re-scans, and the eval
// referee must all inherit the same value; delete throttle-profile.json or pass
// --net-scale to change it, which invalidates the pinned baseline). First scan
// walks the ladder until the baseline LCP fits under the ceiling: apps in the
// normal band stay at x1 Lighthouse realism; only pathologically heavy ones
// (drawdb: 20s = mostly transfer WAIT) scale up so every navigation stops
// costing half a minute. RTT and CPU pressure never change.
const CALIBRATE_LADDER = [1, 2, 4, 8];
const CALIBRATE_CEILING_MS = 8000;
const CANDIDATE_MIN_BYTES = 3 * 1024;
const CANDIDATE_MAX_PAINT_RATIO = 0.02;

// How to spell an invocation in hints: a launcher can dictate it via ROLLDOWN_PERF_KIT_CLI;
// installed, `npx <package name>` when a bin carries the package's bare name (npx runs that
// bin, with or without a local install), else the first bin; from the repo, the script.
const CLI =
  process.env.ROLLDOWN_PERF_KIT_CLI ??
  (/[\\/]node_modules[\\/]/.test(ROOT) ? installedCli() : 'node harness.mjs');

function installedCli() {
  const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));
  const bins = Object.keys(pkg.bin ?? {});
  const bare = pkg.name.split('/').pop();
  return `npx ${bins.includes(bare) ? pkg.name : (bins[0] ?? bare)}`;
}

const kb = (n) => `${(n / 1024).toFixed(1)}KB`;
const kib = (n) => `${(n / 1024).toFixed(1)}KiB`;
const ms = (v) => (v == null ? 'n/a' : `${Math.round(v)}ms`);
const readJson = (file) => (fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8')) : null);
// The state dir self-ignores: agents running `git add -A` in a consumer repo must
// never commit tool state (a haiku run committed the whole Chrome profile).
const guardStateDir = () => {
  const gitignore = path.join(STATE_DIR, '.gitignore');
  if (!fs.existsSync(gitignore)) {
    fs.mkdirSync(STATE_DIR, { recursive: true });
    fs.writeFileSync(gitignore, '*\n');
  }
};
const writeJson = (file, value) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  if (file.startsWith(STATE_DIR)) guardStateDir();
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
};

const cleanups = [];
onInterrupt(async () => {
  for (const fn of cleanups.splice(0)) await fn().catch(() => {});
});

function parse(argv, options) {
  return parseArgs({ args: argv, options, allowPositionals: true }).values;
}

// --- target resolution ---------------------------------------------------------
// Measurement commands operate on a built app ("target"). The first --app/--dist
// is remembered, so every later command can be invoked bare. Each target keeps
// its own state directory: baselines and history never mix across apps.

const TARGET_OPTS = {
  app: { type: 'string' },
  dist: { type: 'string' },
};

// `--bandwidth <Mbit/s> [--rtt <ms>] [--cpu <rate>]`: pin a network in real units - the
// network a deployed environment measured (a record's model) - instead of a net-scale
// step. Pinned like --net-scale; changing it makes the pinned baseline incomparable.
const NETWORK_OPTS = {
  bandwidth: { type: 'string' },
  rtt: { type: 'string' },
  cpu: { type: 'string' },
};

// The throttle identity a measurement will run under: the flag's, else the pinned
// profile's; null before the first calibration (nothing to compare against yet).
function expectedThrottleId(target, opts, networkFlag) {
  if (networkFlag) return throttleId({ network: networkFlag });
  if (opts['net-scale'] != null) return throttleId({ netScale: Number(opts['net-scale']) });
  const profile = readJson(target.paths.throttleProfile);
  return profile ? throttleId(profile) : null;
}

function targetSlug(dist) {
  return dist
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(-60);
}

function isDemoDist(dist) {
  return path.normalize(dist) === path.normalize(path.join(APP_DIR, 'dist'));
}

function targetPaths(dist) {
  const dir = isDemoDist(dist) ? STATE_DIR : path.join(STATE_DIR, 'targets', targetSlug(dist));
  return {
    dir,
    runtimeMetrics: path.join(dir, 'runtime-metrics.json'),
    runtimeState: path.join(dir, '.state.json'),
    runtimeBaseline: path.join(dir, 'baseline.json'),
    coverage: path.join(dir, 'coverage.json'),
    profile: path.join(dir, 'profile.json'),
    history: path.join(dir, 'history.jsonl'),
    throttleProfile: path.join(dir, 'throttle-profile.json'),
    baselineModuleGraph: path.join(dir, 'baseline-module-graph.json'),
    parity: parityPaths(dir),
  };
}

// Resolve --app/<appDir> to an EXISTING build dir, or throw with the fix. A guessed
// path must never be remembered: a haiku agent once obeyed the fabricated path in
// our error message and rewired the app's outDir (then "fixed" the functional check).
function resolveAppDist(appArg) {
  const app = path.resolve(appArg);
  const candidates = ['dist', 'build', 'out'].map((dir) => path.join(app, dir));
  const dist =
    candidates.find((dir) => fs.existsSync(path.join(dir, 'index.html'))) ??
    // --app aimed at the built dir itself is a common miss - accept it.
    (fs.existsSync(path.join(app, 'index.html')) ? app : null);
  if (!dist) {
    throw new Error(
      `no build found under ${app} - tried dist/, build/, out/, and the dir itself (none has an index.html).\n` +
        'Build the app first. --app takes the APP ROOT (the dir you build from); if the build output\n' +
        "lives somewhere else, pass --dist <builtDir> directly. Never change the app's outDir to fit\n" +
        'this tool - aim the tool at the build instead.',
    );
  }
  return dist;
}

function resolveTarget(opts) {
  let dist;
  if (opts.dist) dist = path.resolve(opts.dist);
  else if (opts.app) dist = resolveAppDist(opts.app);
  else {
    const sticky = readJson(TARGET_FILE);
    dist = sticky?.dist ?? path.join(APP_DIR, 'dist');
  }
  if ((opts.dist || opts.app) && readJson(TARGET_FILE)?.dist !== dist) {
    writeJson(TARGET_FILE, { dist, setAtMs: Date.now() });
  }
  console.log(`target: ${dist}${opts.dist || opts.app ? '' : ' (remembered)'}`);
  return { dist, paths: targetPaths(dist), isDemo: isDemoDist(dist) };
}

function expectedFeaturesFor(target, opts) {
  if (opts.features !== undefined) return opts.features.split(',').filter(Boolean);
  return target.isDemo ? FEATURE_NAMES : [];
}

// A command that needs the entry must fail with the REAL problem: "no build here"
// or "could not detect the entry" - never a fabricated default path. The old
// `?? 'main.js'` fallback produced "no sourcemap at <dist>/main.js.map", which an
// agent read as a layout instruction and obeyed.
function requireEntry(target, opts = {}) {
  if (!fs.existsSync(path.join(target.dist, 'index.html'))) {
    throw new Error(
      `no build at ${target.dist} - build the app first.\n` +
        '(Wrong target? --app <appRoot> resolves dist/build/out; --dist <builtDir> aims directly.)',
    );
  }
  const entry = opts.entry ?? detectEntry(target.dist);
  if (!entry) {
    throw new Error(
      `could not detect the entry script in ${path.join(target.dist, 'index.html')} - ` +
        'pass --entry <file> (path relative to the build dir).',
    );
  }
  return entry;
}

/** The entry chunk of a built app, read from its index.html module script. */
function detectEntry(distDir) {
  const indexFile = path.join(distDir, 'index.html');
  if (!fs.existsSync(indexFile)) return null;
  const html = fs.readFileSync(indexFile, 'utf8');
  const locals = [];
  for (const tag of html.match(/<script\b[^>]*>/g) ?? []) {
    const src = tag.match(/\bsrc="([^"]+)"/)?.[1];
    if (!src || src.startsWith('http')) continue;
    // webpack emits cache-busting queries (main.bundle.js?abc123) - strip them
    // or every existsSync/readFile on the entry fails.
    locals.push({
      src: src.split('?')[0].replace(/^\.?\//, ''),
      module: tag.includes('type="module"'),
    });
  }
  // Astro islands have no script tag: each <astro-island> names its chunks in
  // component-url / renderer-url attributes. They load as modules.
  if (!locals.length) {
    for (const [, src] of html.matchAll(/\b(?:component|renderer)-url="([^"]+)"/g)) {
      if (!src.startsWith('http')) {
        locals.push({ src: src.split('?')[0].replace(/^\.?\//, ''), module: false });
      }
    }
  }
  const moduleScript = locals.find((s) => s.module);
  if (moduleScript) return moduleScript.src;
  // webpack-style multi-script pages have no type="module": prefer the
  // main-looking bundle, else the biggest local script on disk (runtime/shim
  // scripts come first in the HTML and are tiny).
  const named = locals.find((s) => /(^|[./])(main|index|app)[^/]*\.m?js$/i.test(s.src));
  if (named) return named.src;
  let best = null;
  for (const s of locals) {
    const file = path.join(distDir, s.src);
    const size = fs.existsSync(file) ? fs.statSync(file).size : -1;
    if (size >= 0 && (!best || size > best.size)) best = { ...s, size };
  }
  return best?.src ?? locals[0]?.src ?? null;
}

/**
 * Resolve the target's PINNED throttle profile, calibrating on first use: walk
 * the net-scale ladder (one probe navigation per step) until the baseline LCP
 * fits under the ceiling, then pin the winning scale. Explicit --net-scale
 * pins without probing; so does --bandwidth/--rtt/--cpu, which pins a network in
 * real units (a deployed environment's measured network) instead of a scale step.
 * Pinned means pinned: every later scan/measure/coverage/
 * profile - and the eval referee - inherits this value until the profile file
 * is deleted or explicitly overridden.
 */
async function ensureThrottleProfile(
  target,
  { origin, cdp, netScaleFlag = null, networkFlag = null },
) {
  const existing = readJson(target.paths.throttleProfile);
  if (networkFlag) {
    const id = throttleId({ network: networkFlag });
    if (!existing || throttleId(existing) !== id) {
      writeJson(target.paths.throttleProfile, {
        schemaVersion: 1,
        network: networkFlag,
        source: 'flag',
        pinnedAtMs: Date.now(),
      });
      console.log(
        `throttle profile pinned by flag: ${id}${existing ? ` (was ${throttleId(existing)} - the pinned baseline is now incomparable, re-pin with scan --pin)` : ''}`,
      );
    }
    return readJson(target.paths.throttleProfile);
  }
  if (netScaleFlag != null) {
    const netScale = Number(netScaleFlag);
    if (!CALIBRATE_LADDER.includes(netScale)) {
      throw new Error(
        `--net-scale must be one of ${CALIBRATE_LADDER.join('/')} (named, reproducible steps only)`,
      );
    }
    if (!existing || throttleId(existing) !== throttleId({ netScale })) {
      writeJson(target.paths.throttleProfile, {
        schemaVersion: 1,
        netScale,
        source: 'flag',
        pinnedAtMs: Date.now(),
      });
      console.log(
        `throttle profile pinned by flag: net x${netScale}${existing ? ` (was ${throttleId(existing)} - the pinned baseline is now incomparable, re-pin with scan --pin)` : ''}`,
      );
    }
    return readJson(target.paths.throttleProfile);
  }
  if (existing) return existing;
  const probes = [];
  let chosen = 1;
  for (const netScale of CALIBRATE_LADDER) {
    const sample = await timedRun(cdp, {
      url: `${origin}/`,
      throttle: scaleThrottle(DEFAULT_THROTTLE, netScale),
      settleMs: 1000,
    });
    const lcpMs = typeof sample.lcp === 'number' ? Math.round(sample.lcp) : null;
    probes.push({ netScale, lcpMs });
    process.stderr.write(`calibrate net x${netScale}: LCP ${ms(sample.lcp)}\n`);
    if (lcpMs == null) break; // measurement failed - stay at the safest default
    chosen = netScale;
    if (lcpMs <= CALIBRATE_CEILING_MS) break;
  }
  const profile = {
    schemaVersion: 1,
    netScale: probes.some((p) => p.lcpMs != null) ? chosen : 1,
    source: 'calibration',
    ceilingMs: CALIBRATE_CEILING_MS,
    probes,
    pinnedAtMs: Date.now(),
  };
  writeJson(target.paths.throttleProfile, profile);
  console.log(
    `throttle profile pinned: net x${profile.netScale} (${probes.map((p) => `x${p.netScale}=${p.lcpMs == null ? 'n/a' : `${p.lcpMs}ms`}`).join(', ')}; ceiling ${CALIBRATE_CEILING_MS}ms)`,
  );
  console.log(
    'this scale is part of the measurement identity - every later scan and the scoring legs use it.',
  );
  return profile;
}

async function withServerAndBrowser(
  target,
  throttleOff,
  fn,
  { netScaleFlag = null, networkFlag = null } = {},
) {
  const distDir = target.dist;
  if (!fs.existsSync(path.join(distDir, 'index.html'))) {
    throw new Error(`no build at ${distDir} - build the app first`);
  }
  const server = await startServer(distDir);
  const browser = await launchBrowser({ profileDir: CHROME_PROFILE_DIR });
  // Called through their owners: these are the SIGINT cleanups, so a `this` that
  // went missing would only surface as a leaked browser/port on interrupt.
  cleanups.push(
    () => server.close(),
    () => browser.close(),
  );
  try {
    let throttle = null;
    if (!throttleOff) {
      const profile = await ensureThrottleProfile(target, {
        origin: server.origin,
        cdp: browser.cdp,
        netScaleFlag,
        networkFlag,
      });
      throttle = throttleFromProfile(profile);
    }
    return await fn({ origin: server.origin, cdp: browser.cdp, throttle });
  } finally {
    await browser.close().catch(() => {});
    await server.close().catch(() => {});
  }
}

// --- measure core ----------------------------------------------------------------

async function gatherSamples(
  cdp,
  origin,
  { throttle, expectedFeatures, runs, warmup, settleMs, earlyStop = null },
) {
  // Navigate '/' (not '/index.html'): SPA routers treat the literal
  // /index.html path as an unknown route and render their 404 page - drawDB
  // showed us a "looking for something?" screen instead of its landing.
  const url = `${origin}/`;
  // The warmup stays even when early stop is possible: it absorbs the
  // launch-cold outlier (first navigation after a browser start), which would
  // otherwise be the single sample the decision rests on.
  for (let i = 0; i < warmup; i++) {
    process.stderr.write(`warmup ${i + 1}/${warmup}...\n`);
    await timedRun(cdp, { url, throttle, expectedFeatures, settleMs });
  }
  const samples = [];
  for (let i = 0; i < runs; i++) {
    const sample = await timedRun(cdp, { url, throttle, expectedFeatures, settleMs });
    samples.push(sample);
    process.stderr.write(`run ${i + 1}/${runs}: LCP ${ms(sample.lcp)}, load ${ms(sample.load)}\n`);
    // Decide on run 1 ONLY: if it lands ambiguous (within the early-stop band),
    // later runs disagreeing with it is exactly the near-threshold case that
    // needs the full median, so no late stopping.
    if (earlyStop && i === 0 && runs > 1 && typeof sample.lcp === 'number') {
      const deltaMs = sample.lcp - earlyStop.baselineLcp;
      if (Math.abs(deltaMs) >= earlyStop.factor * earlyStop.thresholdMs) {
        process.stderr.write(
          `early stop after run 1: |dLCP| ${Math.round(Math.abs(deltaMs))}ms >= ${earlyStop.factor}x noise threshold ${Math.round(earlyStop.thresholdMs)}ms\n`,
        );
        return {
          samples,
          earlyStopped: {
            afterRuns: 1,
            plannedRuns: runs,
            deltaMs: Math.round(deltaMs),
            thresholdMs: Math.round(earlyStop.thresholdMs),
            factor: earlyStop.factor,
          },
        };
      }
    }
  }
  return { samples, earlyStopped: null };
}

function writeMeasureReport(
  target,
  samples,
  { expectedFeatures, label, throttle, earlyStopped = null },
) {
  const summary = summarize(samples, expectedFeatures);
  const prev = readJson(target.paths.runtimeState);
  const baseline = readJson(target.paths.runtimeBaseline);
  // Deltas are only meaningful between measurements taken under the SAME pinned
  // throttle (net scale or network) - it is part of the measurement's identity. A
  // mismatch suppresses the comparison and says so, instead of printing a wrong number.
  const currId = throttle ? throttleId(throttle) : null;
  const sameScale = (state) => state && currId != null && throttleId(state) === currId;
  const chunkGraph = loadChunkGraph(
    target.dist,
    moduleGraphCandidates({
      demoMetricsDir: target.isDemo ? BUILD_METRICS_DIR : undefined,
      dist: target.dist,
    }).map((file) => path.join(path.dirname(file), 'chunk-graph.json')),
  );
  const baselineDelta = sameScale(baseline)
    ? deltaSection(baseline.metrics, summary.metrics)
    : null;
  const report = {
    schemaVersion: 1,
    generatedAtMs: Date.now(),
    label: label || null,
    dist: target.dist,
    entry: detectEntry(target.dist),
    throttle,
    deferred: target.isDemo ? deferredList() : null,
    runs: summary.runs,
    earlyStopped,
    metrics: summary.metrics,
    guard: summary.guard,
    gatingFetches: summary.gatingFetches,
    resourceWeight: summary.resourceWeight,
    renderBlockingGate: summary.renderBlockingGate,
    postLoadScripts: summary.postLoadScripts,
    resourceDetailsComplete: summary.resourceDetailsComplete,
    chunkDiscovery: analyzeChunkDiscovery(chunkGraph, samples, throttle?.latencyMs),
    lcpElement: summary.lcpElement,
    delta: sameScale(prev) ? deltaSection(prev.metrics, summary.metrics) : null,
    baselineDelta,
    paintComparison: comparePaints(baselineDelta, {
      noiseFloorMs: NOISE_FLOOR_MS,
      noiseFloorPct: NOISE_FLOOR_PCT,
    }),
    baselineScaleMismatch:
      baseline && currId != null && !sameScale(baseline)
        ? { pinned: throttleId(baseline), current: currId }
        : null,
    samples: summary.samples,
  };
  writeJson(target.paths.runtimeMetrics, report);
  // runs travels with the state so pinBaseline can refuse single-run pins
  // (quick probes and early-stopped scans - a 1-run baseline poisons deltas);
  // netScale / network travel so future measurements refuse cross-throttle comparisons.
  writeJson(target.paths.runtimeState, {
    schemaVersion: 1,
    tsMs: report.generatedAtMs,
    label: report.label,
    runs: report.runs,
    netScale: throttle && !throttle.network ? (throttle.netScale ?? 1) : undefined,
    network: throttle?.network,
    metrics: report.metrics,
  });
  fs.appendFileSync(
    target.paths.history,
    `${JSON.stringify({
      tsMs: report.generatedAtMs,
      label: report.label,
      deferred: report.deferred,
      metrics: report.metrics,
      guard: report.guard,
    })}\n`,
  );
  return { report, hadBaseline: Boolean(baseline) };
}

function deferredList() {
  const modes = featureModes(APP_DIR);
  return modes ? FEATURE_NAMES.filter((n) => modes[n] === 'deferred') : [];
}

// advice:false drops the "next:" coaching paragraphs (scan prints the verdict right
// after this summary, and every one of them reappears there as the matching lead's
// next: line - printed twice, they sit in the agent's context for the whole session
// and get re-read on every later turn). The signal lines and numbers always print.
function printMeasureSummary(
  report,
  hadBaseline,
  { advice = true, parity = null, lcpCheck = null } = {},
) {
  const m = report.metrics;
  console.log(
    `\n${report.label ? `[${report.label}] ` : ''}${report.runs} runs` +
      `${report.deferred?.length ? `, deferred: ${report.deferred.join(', ')}` : ''}`,
  );
  if (report.earlyStopped) {
    const e = report.earlyStopped;
    console.log(
      `early stop after run 1 of ${e.plannedRuns}: |dLCP vs pinned baseline| ${Math.abs(e.deltaMs)}ms >= ${e.factor}x noise threshold ${e.thresholdMs}ms` +
        ' - this is a probe. A full scan with --no-early-stop must check both FCP and LCP before a decision.',
    );
  }
  console.log(
    `LCP ${ms(m['runtime.lcp_ms'])} (p75 ${ms(m['runtime.lcp_p75_ms'])}) | ` +
      `FCP ${ms(m['runtime.fcp_ms'])} | load ${ms(m['runtime.load_ms'])} | ` +
      `CLS ${m['runtime.cls']} | transfer ${kb(m['runtime.transfer_bytes'])} | ` +
      `${m['runtime.js_request_count']} js requests` +
      `${report.throttle && (report.throttle.network || (report.throttle.netScale ?? 1) !== 1) ? ` | ${throttleId(report.throttle)} (pinned)` : ''}`,
  );
  if (report.baselineScaleMismatch) {
    const mm = report.baselineScaleMismatch;
    console.log(
      `vs pinned baseline: INCOMPARABLE - the baseline was pinned under ${mm.pinned}, this measurement ran under ${mm.current}.`,
    );
    console.log('next: re-pin under the current profile (scan --pin) before judging any change.');
  }
  const guardOk =
    report.guard.allFeaturesReady &&
    report.guard.heroRendered !== false && // null = no hero probe on this app
    report.guard.fcpObservedInAllRuns &&
    report.guard.lcpObservedInAllRuns;
  console.log(`guard: ${guardOk ? 'PASS' : `FAIL ${JSON.stringify(report.guard)}`}`);

  // Deep signals: things LCP alone doesn't say, each with the move it suggests.
  const gate = report.renderBlockingGate;
  if (gate?.gating) {
    console.log(
      `render-blocking CSS gate: ${gate.count} blocking stylesheet(s), ${gate.kb}KB held ${Math.round(gate.shareOfFcp * 100)}% of the FCP timeline (last finished ${gate.lastEndMs}ms, FCP ${gate.fcpMs}ms).`,
    );
    for (const w of (gate.worst ?? []).slice(0, 3)) {
      console.log(`  blocking until ${w.end}ms: ${w.name} (${kb(w.bytes)})`);
    }
    if (advice) {
      console.log(
        '  next: nothing paints until these finish - inline the small critical CSS, load the rest',
      );
      console.log(
        '  non-blocking (preload + media swap), and split styles only later routes need. Fix this',
      );
      console.log('  and any render-gating fetch before judging CPU/JS deferrals.');
    }
  }
  const renderGap = m['runtime.render_gap_ms'];
  if (typeof renderGap === 'number' && renderGap > 150) {
    console.log(
      `render gap: LCP landed ${Math.round(renderGap)}ms AFTER the load event. Downloads can continue after load.`,
    );
    for (const evidence of renderGapEvidence(report)) {
      console.log(`  candidate: ${evidence.detail}`);
      if (advice) console.log(`  next: ${evidence.next}`);
    }
    if (advice) {
      console.log(
        `  next: check these candidates against the source and the CPU profile (${CLI} profile). Check whether an animation hides the LCP element. Timing alone does not show what blocked paint.`,
      );
    }
  }
  for (const paint of ['fcp', 'lcp']) {
    const discovery = report.chunkDiscovery?.[paint];
    if (!discovery?.groups.length) continue;
    console.log(
      `late chunk candidates before ${paint.toUpperCase()}: ${discovery.groups.length} static import group(s). Paint dependencies are unverified.`,
    );
    for (const group of discovery.groups.slice(0, 4)) {
      console.log(
        `  ${group.parent}: ${group.count} chunk(s), ${kib(group.bytes)}, up to ${ms(group.maxDelayMs)} delay (${group.runsAffected}/${group.measuredRuns} runs).`,
      );
      const example = group.example.chunks[0];
      console.log(`    import chain: ${example.importChain.join(' -> ')}`);
      if (example.facadeModuleId) console.log(`    source: ${example.facadeModuleId}`);
    }
    console.log('  Groups and paint windows overlap. Do not add their bytes or delays.');
    if (advice) console.log(`  next: ${chunkDiscoveryAdvice(paint)}`);
    console.log(
      `  evidence: ${discovery.graphFile}; runtime-metrics.json -> chunkDiscovery.${paint}.groups`,
    );
  }
  const prepaintCpu = m['runtime.prepaint_longtask_ms'];
  if (typeof prepaintCpu === 'number' && prepaintCpu > 150) {
    console.log(`pre-paint CPU: ${Math.round(prepaintCpu)}ms of long tasks before first paint.`);
    if (advice) {
      console.log(
        `  next: run ${CLI} profile to find the modules that use this CPU time. Check their timing against required downloads. Test deferring work that the first paint does not need. Compare both FCP and LCP.`,
      );
    }
  }

  if (report.baselineDelta) {
    const comparison = report.paintComparison;
    for (const [paint, metric] of Object.entries(comparison.metrics)) {
      if (metric.state === 'unknown') {
        console.log(`vs pinned baseline: ${paint.toUpperCase()} missing`);
        continue;
      }
      console.log(
        `vs pinned baseline: ${paint.toUpperCase()} ${metric.delta > 0 ? '+' : ''}${Math.round(metric.delta)}ms ` +
          `(${metric.pct > 0 ? '+' : ''}${metric.pct}%) -> ${metric.state} (noise threshold ${Math.round(metric.thresholdMs)}ms)`,
      );
    }
    if (parity?.state === 'open' || !guardOk) {
      console.log(
        'next: resolve the failed guard or rendered output check before judging paint gains. Both paints must appear in every run.',
      );
    } else if (report.runs < 3) {
      console.log(
        `next: this is a probe. Run ${CLI} scan --no-early-stop with at least 3 runs before a keep/revert decision.`,
      );
    } else if (comparison.state === 'improved') {
      console.log(
        `next: one or both paint metrics improved beyond noise, and neither regressed beyond noise. Keep the change if the functional check passes. Re-pin with ${CLI} baseline. Run scan --parity when the work is complete.`,
      );
    } else if (comparison.state === 'regressed') {
      console.log(
        'next: FCP or LCP regressed beyond noise. Revert the change and test a different experiment.',
      );
    } else if (comparison.state === 'unknown') {
      console.log('next: collect both FCP and LCP before judging this change.');
    } else {
      console.log(
        'next: neither FCP nor LCP improved beyond noise. Revert the change and test a different experiment.',
      );
    }
  } else if (hadBaseline) {
    console.log('vs pinned baseline: n/a (metrics missing or throttle changed)');
  }
  // Free on every measurement (the timed runs already observe it), so it is shown
  // as it happens - information, not a gate; the final parity check gates on it.
  if (lcpCheck?.state === 'same') {
    console.log(`LCP element: same as the original - ${describeLcp(lcpCheck.current)}`);
  } else if (lcpCheck?.state === 'unstable') {
    console.log(
      `LCP element: varies between loads (${describeLcp(lcpCheck.anchor)} / ${describeLcp(lcpCheck.current)})`,
    );
  } else if (lcpCheck && lcpCheck.state !== 'unknown') {
    console.log(
      `LCP element: ${lcpReason(lcpCheck)} - LCP now times a different paint than the original did, so this gain is likely not real (the final rendered-output check opens on it).`,
    );
  }
  if (report.delta?.['runtime.lcp_ms']) {
    const d = report.delta['runtime.lcp_ms'];
    console.log(
      `vs previous measure: LCP ${d.delta > 0 ? '+' : ''}${Math.round(d.delta)}ms (${d.pct > 0 ? '+' : ''}${d.pct}%)`,
    );
  }
}

function pinBaseline(target) {
  const state = readJson(target.paths.runtimeState);
  if (!state) throw new Error('nothing to pin - measure (or scan) first');
  // A 1-run median (quick probe or early-stopped scan) poisons every later
  // delta. scan --pin always runs full sampling - use it.
  if (state.runs === 1) {
    throw new Error(
      'refusing to pin a single-run measurement - run a full scan first (scan --pin re-measures with full sampling and then pins)',
    );
  }
  // A build already KNOWN to render differently from the original must never become
  // the reference later gains count from. An unchecked build may be pinned: the
  // comparison runs once, when the optimizing is done.
  const parity = parityStatus(target);
  if (parity.state === 'open') {
    throw new Error(
      'refusing to pin: this build renders differently from the original (see the verdict). Fix or revert the change that caused it.\n' +
        `If a HUMAN decides the difference is intended, they approve it: ${CLI} parity approve <page> --reason "<why>"`,
    );
  }
  if (parity.report && !parity.report.integrity) {
    throw new Error(
      'refusing to pin: the saved original build was modified after it was saved - parity cannot vouch for this build.',
    );
  }
  writeJson(target.paths.runtimeBaseline, state);
  console.log(
    `baseline pinned (label: ${state.label ?? 'none'}, LCP ${ms(state.metrics['runtime.lcp_ms'])})`,
  );
  if (target.isDemo) {
    const buildState = path.join(BUILD_METRICS_DIR, '.state.json');
    if (fs.existsSync(buildState)) {
      fs.copyFileSync(buildState, path.join(BUILD_METRICS_DIR, 'baseline.json'));
      console.log(`build baseline pinned (${path.join(BUILD_METRICS_DIR, 'baseline.json')})`);
    }
  }
  snapshotBaselineGraph(target);
}

// Alongside the runtime baseline, snapshot the resolved module-graph.json so `graph-diff`
// can attribute later changes to the eager set. The WHOLE sidecar is copied (no field
// cherry-picking - the v4.5 lesson); a stale/absent graph just skips it with one line.
// This runs from the single pin helper, so every pin path (baseline / scan --pin /
// measure --pin / first-scan auto-pin) gets the snapshot.
function snapshotBaselineGraph(target) {
  const entry = detectEntry(target.dist);
  const builtAtMs =
    entry && fs.existsSync(path.join(target.dist, entry))
      ? fs.statSync(path.join(target.dist, entry)).mtimeMs
      : null;
  const mg = moduleGraphStatus(target, builtAtMs);
  if (mg.state === 'present') {
    fs.mkdirSync(path.dirname(target.paths.baselineModuleGraph), { recursive: true });
    guardStateDir();
    fs.copyFileSync(mg.graph.file, target.paths.baselineModuleGraph);
    console.log(`baseline module graph snapshotted (graph-diff will compare against it)`);
  } else if (mg.state === 'stale') {
    console.log(
      'note: module graph predates this build - no graph snapshot pinned (graph-diff needs a fresh one).',
    );
  } else if (mg.state === 'absent-rolldown') {
    console.log(
      'note: no module graph collected - enable the devtools report mode to pin one for graph-diff.',
    );
  }
}

// --- coverage core ----------------------------------------------------------------

function buildCoverageReport(target, cov, entry) {
  // Fetch-timing map (decoded pathname -> bytes) for scripts whose download
  // began before first paint - see attributeChunks' static-prepaint-transfer.
  const prePaintFetches = new Map();
  for (const fetch of cov.scriptFetches ?? []) {
    if (!fetch.prePaint) continue;
    let file = fetch.pathname;
    try {
      file = decodeURIComponent(file);
    } catch {
      /* keep raw */
    }
    prePaintFetches.set(file.replace(/^\/+/, ''), fetch.bytes ?? 0);
  }
  const { chunks, modules, skipped } = attributeChunks({
    scripts: cov.scripts,
    entryName: `/${entry.replaceAll('\\', '/')}`,
    prePaintFetches,
    readChunk: (file) => {
      const chunkFile = path.join(target.dist, file);
      if (!fs.existsSync(chunkFile) || !fs.existsSync(`${chunkFile}.map`)) return null;
      return {
        code: fs.readFileSync(chunkFile, 'utf8'),
        map: JSON.parse(fs.readFileSync(`${chunkFile}.map`, 'utf8')),
      };
    },
  });

  // Defer candidates. For the demo app these map to feature marker blocks (the
  // seams `defer <name>` can rewrite); otherwise they are advisory per-module.
  const modes = target.isDemo ? featureModes(APP_DIR) : null;
  const candidates = [];
  for (const mod of modules) {
    if (mod.totalBytes < CANDIDATE_MIN_BYTES || mod.paintRatio >= CANDIDATE_MAX_PAINT_RATIO)
      continue;
    const feature = FEATURE_NAMES.find((n) => mod.source.endsWith(`features/${n}.ts`));
    if (modes) {
      if (!feature || modes[feature] !== 'baseline') continue;
      candidates.push({ feature, ...mod });
    } else {
      candidates.push({ feature: feature ?? null, ...mod });
    }
  }

  const report = {
    schemaVersion: 1,
    generatedAtMs: Date.now(),
    entry,
    chunks,
    skippedChunks: skipped,
    deferred: target.isDemo ? deferredList() : null,
    thresholds: {
      candidateMinBytes: CANDIDATE_MIN_BYTES,
      candidateMaxPaintRatio: CANDIDATE_MAX_PAINT_RATIO,
    },
    modules,
    candidates,
    coldAtPaint: coldAtPaintModules(modules).slice(0, 20),
  };
  writeJson(target.paths.coverage, report);
  return report;
}

// compact:true is the re-scan view: the actionable sections (chunks, candidates, cold
// bytes, large-at-paint, sibling groups) stay complete, only the informational module
// table shrinks - it is the biggest block of a scan and re-printing 60 rows on every
// iteration multiplies what an agent re-reads each turn for the rest of its session.
// advice:false drops the "next:" paragraphs for the same reason as printMeasureSummary:
// scan prints the verdict right after, where every one of them reappears as a lead's
// next: line. Standalone `coverage` has no verdict following it and keeps them.
function printCoverageReport(target, report, { compact = false, advice = true } = {}) {
  const { modules, candidates, entry } = report;
  const chunks = report.chunks ?? [];
  const extraChunks = chunks.filter((c) => !c.entry);
  const chunkTag = (mod) =>
    extraChunks.some((c) => c.file === mod.chunk) ? `  [${mod.chunk}]` : '';
  console.log(
    `initial-load coverage (${entry}${extraChunks.length ? ` + ${extraChunks.length} pre-paint chunk(s)` : ''}, first paint vs settled):\n`,
  );
  if (extraChunks.length) {
    console.log(
      '  pre-paint chunks - fetched AND executed before first paint, so critical-path transfer:',
    );
    for (const c of extraChunks) {
      console.log(`    ${c.file}  ${kb(c.totalBytes)}  paint ${(c.paintRatio * 100).toFixed(0)}%`);
    }
  }
  for (const s of report.skippedChunks ?? []) {
    if (s.reason === 'no-sourcemap') {
      console.log(
        `  NOTE: chunk ${s.file} executed before paint but has no sourcemap - its bytes are NOT attributed below`,
      );
    }
  }
  const staticPrepaint = (report.skippedChunks ?? [])
    .filter((s) => s.reason === 'static-prepaint-transfer')
    .sort((a, b) => (b.bytes ?? 0) - (a.bytes ?? 0));
  if (staticPrepaint.length) {
    const totalBytes = staticPrepaint.reduce((sum, s) => sum + (s.bytes ?? 0), 0);
    console.log(
      `  STATIC PRE-PAINT TRANSFER: ${staticPrepaint.length} chunk(s), ${kb(totalBytes)} - fetched BEFORE first paint (static tags/preloads)`,
    );
    console.log(
      '  but executed only after it. Their download competes with the paint for bandwidth. Largest:',
    );
    for (const s of staticPrepaint.slice(0, 10)) {
      console.log(
        `    ${s.file}  ${kb(s.bytes ?? 0)}${s.neverExecuted ? '  (never executed by settle at all)' : ''}`,
      );
    }
    if (staticPrepaint.length > 10)
      console.log(`    ... +${staticPrepaint.length - 10} more in coverage.json`);
    if (advice) {
      console.log(
        '  next: load these on demand (dynamic import / drop them from the initial script tags or preloads)',
      );
      console.log('  so the first paint stops paying for their transfer.');
    }
  }
  const lazyChunks = (report.skippedChunks ?? []).filter((s) => s.reason === 'post-paint');
  if (lazyChunks.length) {
    console.log(
      `  (${lazyChunks.length} chunk(s) fetched AND first executed after paint - already deferred, not analyzed: ${lazyChunks.map((s) => s.file).join(', ')})`,
    );
  }
  if (extraChunks.length || lazyChunks.length || staticPrepaint.length) console.log('');
  const pct = (r) => `${(r * 100).toFixed(1)}%`;
  const shown = modules.slice(0, compact ? 15 : 60);
  for (const mod of shown) {
    const verdict =
      mod.paintRatio >= CANDIDATE_MAX_PAINT_RATIO
        ? 'used-before-paint'
        : mod.settleRatio >= CANDIDATE_MAX_PAINT_RATIO
          ? 'post-paint-only'
          : 'not-executed-by-settle';
    console.log(
      `  ${mod.source.padEnd(34)} ${kb(mod.totalBytes).padStart(9)}  ` +
        `paint ${pct(mod.paintRatio).padStart(6)}  settle ${pct(mod.settleRatio).padStart(6)}  ${verdict}${chunkTag(mod)}`,
    );
  }
  if (modules.length > shown.length) {
    console.log(
      `  ... +${modules.length - shown.length} more modules (largest shown) - full list in coverage.json` +
        (compact ? ', full table with scan --full' : ''),
    );
  }
  if (candidates.length) {
    const shownCandidates = candidates.slice(0, 12);
    console.log(
      `\ndefer candidates (>=${kb(CANDIDATE_MIN_BYTES)}, <${CANDIDATE_MAX_PAINT_RATIO * 100}% executed at paint), largest first:`,
    );
    for (const c of shownCandidates) {
      console.log(
        `  ${c.feature ?? c.source}  (${kb(c.totalBytes)}${chunkTag(c) ? `, in ${c.chunk}` : ''})` +
          `${c.feature ? `  -> node harness.mjs defer ${c.feature}` : ''}`,
      );
    }
    if (candidates.length > shownCandidates.length) {
      console.log(`  ... +${candidates.length - shownCandidates.length} more in coverage.json`);
    }
    if (advice) {
      console.log(
        target.isDemo
          ? '\nnext: defer the top candidate, rebuild, then measure.'
          : '\nnext: change the app so the landing page stops loading these before first paint\n' +
              '(follow their import chains from the entry), rebuild, run its functional check, then measure.',
      );
    }
  } else {
    console.log(
      '\nno defer candidates at current thresholds - nothing sizeable is parsed-but-unexecuted.',
    );
  }

  // The unified byte view: candidates catch the never-ran extreme, but a module
  // that PARTIALLY executes at boot (vendor SDK init) matches no other bucket
  // while holding the most recoverable weight. Rank by cold bytes so it can't hide.
  const cold = report.coldAtPaint ?? coldAtPaintModules(modules);
  if (cold.length) {
    const shownCold = cold.slice(0, 12);
    console.log(
      '\ncold bytes at paint - fetched+parsed before first paint but mostly unread by it, coldest first:',
    );
    for (const mod of shownCold) {
      console.log(
        `  ${kb(mod.coldBytes).padStart(9)} cold  (${kb(mod.totalBytes)} @ ${(mod.paintRatio * 100).toFixed(0)}% at paint)  ${mod.source}` +
          `${chunkTag(mod) ? `  [${mod.chunk}]` : ''}${mod.framework ? '  (framework runtime - import edge rarely movable)' : ''}`,
      );
    }
    if (cold.length > shownCold.length)
      console.log(`  ... +${cold.length - shownCold.length} more in coverage.json`);
    if (advice) {
      console.log(
        'next: for each non-framework module, find the import edge that pulls it in before paint and move',
      );
      console.log(
        'that edge behind interaction/idle (dynamic import). A middling percentage on a vendor SDK usually',
      );
      console.log(
        'means one boot-time init call drags the whole package - defer the call, not just the import.',
      );
    }
  }

  // Executed-at-paint is NOT the same as needed-at-paint: top-level data counts
  // as "executed" the moment its module is imported. Surface the places where
  // that inversion typically hides real weight.
  const largeHot = largeAtPaintModules(modules);
  if (largeHot.length) {
    console.log(
      '\nlarge modules fully executed at paint - "executed" does NOT prove the first paint needs their contents (top-level data evaluates on import):',
    );
    for (const mod of largeHot) {
      console.log(
        `  ${mod.source}  (${kb(mod.totalBytes)}, ${(mod.paintRatio * 100).toFixed(0)}% at paint${chunkTag(mod) ? `, in ${mod.chunk}` : ''})`,
      );
    }
    if (advice) {
      console.log(
        'next: for each, check how much of it the first render actually reads; split rarely-read parts (full records, long bodies, alternate variants) into a module reached only by dynamic import.',
      );
    }
  }

  for (const group of siblingVariantGroups(modules)) {
    console.log(
      `\nsibling group ${group.dir}: ${group.files} modules, ${kb(group.bytes)}, ~${Math.round((group.paintBytes / group.bytes) * 100)}% executed at paint.`,
    );
    if (advice) {
      console.log(
        'next: families of same-shaped modules (locales, themes, per-tenant configs) usually need only ONE variant per session - keep the default in the entry and load the active variant with a dynamic import.',
      );
    }
  }
}

// --- profile core ----------------------------------------------------------------

function buildProfileReport(target, profile, entry) {
  const summary = aggregateProfile(profile, {
    readChunk: (file) => {
      const chunkFile = path.join(target.dist, file);
      if (!fs.existsSync(chunkFile) || !fs.existsSync(`${chunkFile}.map`)) return null;
      return {
        code: fs.readFileSync(chunkFile, 'utf8'),
        map: JSON.parse(fs.readFileSync(`${chunkFile}.map`, 'utf8')),
      };
    },
  });
  const report = { schemaVersion: 2, generatedAtMs: Date.now(), entry, ...summary };
  writeJson(target.paths.profile, report);
  return report;
}

function printProfileReport(report, { advice = true } = {}) {
  console.log(`boot CPU by module, navigation -> FCP observation (${report.totalMs}ms sampled):\n`);
  for (const row of report.rows.slice(0, 20)) {
    const pctStr =
      report.totalMs > 0 ? `${((row.ms / report.totalMs) * 100).toFixed(0).padStart(4)}%` : '';
    console.log(
      `  ${row.bucket.padEnd(40)} ${String(row.ms).padStart(7)}ms ${pctStr} [${row.kind}]`,
    );
  }
  console.log(
    `\nsource ${report.attribution.sourceMs}ms | engine ${report.attribution.engineMs}ms | unattributed ${report.attribution.unattributedMs}ms`,
  );
  console.log(
    'The profile can include work between FCP and its observation. Sampled time is not a predicted paint gain.',
  );
  if (advice) {
    console.log(
      '\nnext: check whether each source module contributes to the first render. Test deferring only optional work. Compare both FCP and LCP.',
    );
    if (profileEvidence(report).unattributed.length) {
      console.log(
        'Inspect unattributed rows and supply missing sourcemaps before treating CPU attribution as complete.',
      );
    }
  }
}

// --- verdict core ----------------------------------------------------------------
// The lesson behind this command: a diagnostic tool's completeness claim is
// load-bearing — an agent that trusts a premature "converged" stops in front of
// real wins. `verdict` therefore fuses EVERY signal class, refuses to conclude
// while any lead is open or any signal is missing/stale, and states the tools'
// blind-spot boundary even when everything is clear.

function printVerdict(target, opts = {}) {
  const entry = requireEntry(target, opts);
  const entryFile = path.join(target.dist, entry);
  const builtAtMs = fs.statSync(entryFile).mtimeMs;
  // A report vouches only for the build it ran against: entry filename must match
  // (hashed names change with content) and it must postdate the current build.
  const fresh = (report) =>
    Boolean(report) &&
    (!report.entry || report.entry === entry) &&
    report.generatedAtMs >= builtAtMs;

  const runtime = readJson(target.paths.runtimeMetrics);
  const coverage = readJson(target.paths.coverage);
  const profile = readJson(target.paths.profile);

  const lines = [];
  let openCount = 0;
  let unknownCount = 0;
  let pendingCount = 0;
  // pending = a check that belongs at the END of the loop (rendered-output parity):
  // it blocks "done" like UNKNOWN does, but says it is not wanted per change.
  const lead = (state, title, detail, next) => {
    if (state === 'open') openCount++;
    if (state === 'unknown') unknownCount++;
    if (state === 'pending') pendingCount++;
    const tag = {
      open: '[OPEN]   ',
      unknown: '[UNKNOWN]',
      pending: '[PENDING]',
      clear: '[clear]  ',
    }[state];
    lines.push(`  ${tag} ${title}${detail ? ` - ${detail}` : ''}`);
    if (next && state !== 'clear') lines.push(`            next: ${next}`);
  };

  // Rendered output first: it decides whether any number below counts. It is
  // compared ONCE, when the optimizing is done; a difference starts recover mode.
  const parity = parityStatus(target);
  const recover = readJson(target.paths.parity.recover);
  const parityTitle = 'rendered output vs the original build';
  const recheck =
    recover?.strategy === 'each-tweak'
      ? `every scan re-checks (strategy each-tweak)`
      : `\`${CLI} parity\` (all pages; --only <page> re-checks just the broken ones)`;
  if (parity.state === 'none') {
    lead(
      'unknown',
      parityTitle,
      'no original build saved for this target',
      `${CLI} scan - the first scan saves the CURRENT build as the original, so run it before changing anything`,
    );
  } else if (parity.report && !parity.report.integrity) {
    lead(
      'unknown',
      parityTitle,
      'the saved original build was modified after it was saved',
      `a human builds the original commit and saves it again: ${CLI} parity anchor --replace`,
    );
  } else if (parity.state === 'open') {
    const open = parity.report.pages.filter((p) => p.state === 'open');
    lead(
      'open',
      `rendered output differs from the original on ${open.length}/${parity.manifest.pages.length} page(s)`,
      open.map((p) => `${p.page}: ${p.reasons.join('; ')}`).join(' | '),
      `RECOVER MODE - this build's LCP numbers do not count. Fix or revert the change that caused it (diff images: ${target.paths.parity.pages}); re-check with ${recheck}. Options: \`${CLI} parity recover\``,
    );
  } else if (parity.state === 'stale' || parity.state === 'partial') {
    const checked = parity.state === 'partial' ? parity.report.pages.length : 0;
    if (recover) {
      lead(
        'unknown',
        parityTitle,
        `RECOVER MODE - ${checked ? `only ${checked}/${parity.manifest.pages.length} page(s) re-checked for this build` : 'this build is not re-checked yet'}`,
        `re-check with ${recheck}; recover mode ends when every page matches`,
      );
    } else if (checked) {
      lead(
        'unknown',
        parityTitle,
        `only ${checked} of ${parity.manifest.pages.length} page(s) compared for this build`,
        `${CLI} parity`,
      );
    } else {
      lead(
        'pending',
        parityTitle,
        'compared once, when you have finished optimizing - not needed per change',
        `when you are done: ${CLI} scan --parity  (the final measurement + the comparison on every pinned page)`,
      );
    }
  } else {
    const pages = parity.report?.pages ?? [];
    const unstable = pages.filter((p) => p.state === 'unknown');
    // A check made without a fresh timed measurement could not compare the LCP
    // element; a measurement taken since (same build) can, with no capture.
    const lcpLate =
      parity.state !== 'identical' && !parity.report.lcpCompared && fresh(runtime)
        ? lateLcpCheck(parity.manifest, runtime)
        : null;
    if (unstable.length) {
      lead(
        'unknown',
        parityTitle,
        unstable.map((p) => `${p.page}: ${p.notes[0] ?? 'not judged'}`).join(' | '),
        `a human declares masks for the parts that move: ${CLI} parity --mask <selector>`,
      );
    } else if (lcpLate && ['changed', 'shrank', 'placeholder'].includes(lcpLate.state)) {
      lead(
        'open',
        'rendered output: the LCP element is not the original one',
        lcpReason(lcpLate),
        `RECOVER MODE - LCP now times a cheaper paint, so the gain does not count. Fix or revert the change that caused it; re-check with ${CLI} scan --parity`,
      );
    } else if (parity.state !== 'identical' && !parity.report.lcpCompared && !lcpLate) {
      lead(
        'unknown',
        parityTitle,
        `same on ${pages.length} page(s), but the LCP element was not compared (no fresh timed measurement of this build)`,
        `${CLI} scan`,
      );
    } else {
      const same = pages.filter((p) => p.state === 'clear').length;
      lead(
        'clear',
        parityTitle,
        `${parity.state === 'identical' ? 'this build is byte-identical to the original' : `same on ${same} of ${pages.length} page(s)`}${parityExtras(parity.report)}`,
      );
    }
  }

  if (!fresh(runtime)) {
    lead(
      'unknown',
      'render gap / pre-paint CPU',
      runtime ? 'measurement is stale (dist was rebuilt after it)' : 'no measurement yet',
      `${CLI} measure --runs 5 --label <name>  (or scan)`,
    );
  } else {
    const gate = runtime.renderBlockingGate;
    if (gate?.gating) {
      lead(
        'open',
        `render-blocking CSS gates first paint (${gate.count} stylesheet(s), ${gate.kb}KB)`,
        `held ${Math.round(gate.shareOfFcp * 100)}% of the FCP timeline (finished ${gate.lastEndMs}ms, FCP ${gate.fcpMs}ms) - nothing painted until it arrived: ${(
          gate.worst ?? []
        )
          .slice(0, 2)
          .map((w) => `${w.name} ${kb(w.bytes)}`)
          .join(', ')}`,
        'inline critical CSS, load the rest non-blocking (preload + media swap), split route-only styles - fix this before judging CPU/JS deferrals',
      );
    } else {
      lead(
        'clear',
        'render-blocking CSS',
        gate
          ? `blocking CSS held only ${Math.round((gate.shareOfFcp ?? 0) * 100)}% of the FCP timeline - not the gate`
          : 'none observed',
      );
    }

    const gap = runtime.metrics['runtime.render_gap_ms'];
    if (typeof gap === 'number' && gap > 150) {
      lead(
        'open',
        `LCP after load: ${Math.round(gap)}ms`,
        'downloads, CPU work, or visibility changes can delay paint',
        `Inspect all candidates below. Check the source and the CPU profile (${CLI} profile). Check whether an animation hides the LCP element.`,
      );
      for (const evidence of renderGapEvidence(runtime)) {
        lines.push(`            candidate: ${evidence.detail}`);
        lines.push(`            next: ${evidence.next}`);
      }
    } else {
      lead(
        gap == null ? 'unknown' : 'clear',
        'LCP after load',
        gap == null ? 'not measurable' : `LCP lands ${Math.round(gap)}ms after load`,
        `${CLI} scan`,
      );
    }

    for (const paint of ['fcp', 'lcp']) {
      const discovery = runtime.chunkDiscovery?.[paint];
      lead(
        discovery?.state ?? 'unknown',
        `late chunk candidates before ${paint.toUpperCase()}`,
        !discovery
          ? 'runtime report has no discovery data for this paint'
          : (discovery.reason ??
              (discovery.groups.length
                ? `${discovery.groups.length} static group(s); paint dependencies are unverified. Largest: ${discovery.groups[0].parent}, ${kib(discovery.groups[0].bytes)}`
                : `${discovery.matchedScripts} scripts matched; no static descendant met the delay threshold before this paint. Dynamic imports are excluded.`)),
        discovery?.state === 'open'
          ? `Inspect runtime-metrics.json -> chunkDiscovery.${paint}.groups. ${chunkDiscoveryAdvice(paint)} Groups overlap. Delay is not a predicted paint gain.`
          : 'run scan with full request details and a current chunk-graph.json or Vite manifest',
      );
    }

    const cpu = runtime.metrics['runtime.prepaint_longtask_ms'];
    if (typeof cpu !== 'number' || cpu <= 150) {
      lead(
        'clear',
        'pre-paint CPU',
        cpu == null
          ? 'no long tasks observed'
          : `${Math.round(cpu)}ms of long tasks before paint (baseline territory)`,
      );
    } else if (!fresh(profile)) {
      lead(
        'unknown',
        `pre-paint CPU ${Math.round(cpu)}ms`,
        profile ? 'profile is stale (dist was rebuilt after it)' : 'not yet attributed to modules',
        `${CLI} profile  (or scan)`,
      );
    } else {
      const evidence = profileEvidence(profile);
      lead(
        evidence.state,
        `pre-paint CPU ${Math.round(cpu)}ms`,
        evidence.reason ??
          (evidence.candidates.length
            ? `sampled source work near FCP: ${evidence.candidates
                .slice(0, 3)
                .map((row) => `${row.bucket} ${row.ms}ms`)
                .join(', ')}`
            : evidence.unattributed.length
              ? 'sampled work lacks complete source attribution'
              : 'no source module reaches the 15ms sample threshold'),
        'Check which work the first render needs. Test deferring optional work and compare both FCP and LCP. Run profile again for old reports.',
      );
      if (evidence.unattributed.length) {
        lead(
          'unknown',
          'CPU source attribution',
          `${profile.attribution.unattributedMs}ms unattributed: ${evidence.unattributed
            .slice(0, 3)
            .map((row) => `${row.bucket} [${row.kind}]`)
            .join(', ')}`,
          'Inspect profile.json. Supply missing sourcemaps or inspect generated/inline code; do not classify these rows as engine work.',
        );
      }
    }
  }

  if (!fresh(coverage)) {
    lead(
      'unknown',
      'coverage (cold-at-paint / candidates / large-at-paint / sibling groups)',
      coverage ? 'coverage report is stale (dist was rebuilt after it)' : 'no coverage run yet',
      `${CLI} coverage  (or scan)`,
    );
  } else {
    const candidates = coverage.candidates ?? [];
    const inChunk = (c) =>
      coverage.chunks?.some((ch) => !ch.entry && ch.file === c.chunk) ? ` [${c.chunk}]` : '';
    if (candidates.length) {
      lead(
        'open',
        `defer candidates (${candidates.length})`,
        candidates
          .slice(0, 3)
          .map((c) => `${c.feature ?? c.source} ${kb(c.totalBytes)}${inChunk(c)}`)
          .join(', '),
        'lazy-load them, rebuild, re-measure',
      );
    } else {
      lead('clear', 'defer candidates', 'nothing sizeable is parsed-but-unexecuted');
    }

    const cold = (coverage.coldAtPaint ?? coldAtPaintModules(coverage.modules ?? [])).filter(
      (m) => !m.framework,
    );
    const coldOpen = cold.filter((m) => m.coldBytes >= COLD_OPEN_MIN_BYTES);
    if (coldOpen.length) {
      lead(
        'open',
        `cold bytes at paint (${coldOpen.length} module(s) >=${kb(COLD_OPEN_MIN_BYTES)} cold)`,
        coldOpen
          .slice(0, 4)
          .map(
            (m) =>
              `${m.source} ${kb(m.coldBytes)} cold of ${kb(m.totalBytes)} @ ${(m.paintRatio * 100).toFixed(0)}%${inChunk(m)}`,
          )
          .join(', '),
        'move their import edges behind interaction/idle; a partially-executed vendor SDK usually hides one boot-time init call - defer the call, not just the import',
      );
    } else {
      lead(
        'clear',
        'cold bytes at paint',
        `no non-framework module holds >=${kb(COLD_OPEN_MIN_BYTES)} unread at paint`,
      );
    }
    const unattributed = (coverage.skippedChunks ?? []).filter((s) => s.reason === 'no-sourcemap');
    if (unattributed.length) {
      lead(
        'unknown',
        `unattributed pre-paint chunk(s): ${unattributed.map((s) => s.file).join(', ')}`,
        'executed before first paint but built without a sourcemap - their bytes are invisible to coverage',
        'rebuild with sourcemaps for these chunks, then re-run coverage',
      );
    }

    const staticPre = (coverage.skippedChunks ?? [])
      .filter((s) => s.reason === 'static-prepaint-transfer')
      .sort((a, b) => (b.bytes ?? 0) - (a.bytes ?? 0));
    const staticPreBytes = staticPre.reduce((sum, s) => sum + (s.bytes ?? 0), 0);
    if (staticPreBytes >= 100 * 1024) {
      lead(
        'open',
        `static pre-paint transfer (${staticPre.length} chunk(s), ${kb(staticPreBytes)})`,
        `fetched before paint, executed after it - the paint paid for the download: ${staticPre
          .slice(0, 3)
          .map((s) => `${s.file} ${kb(s.bytes ?? 0)}`)
          .join(', ')}`,
        'make these load on demand (dynamic import / drop from initial script tags or preloads) - biggest lever when transfer dominates LCP',
      );
    } else {
      lead(
        'clear',
        'static pre-paint transfer',
        staticPre.length
          ? `only ${kb(staticPreBytes)} fetched-but-unused before paint`
          : 'nothing fetched before paint that the paint does not use',
      );
    }

    const largeHot = largeAtPaintModules(coverage.modules ?? []);
    if (largeHot.length) {
      lead(
        'open',
        `large modules executed at paint (${largeHot.length})`,
        largeHot
          .slice(0, 3)
          .map((m) => `${m.source} ${kb(m.totalBytes)}`)
          .join(', '),
        'executed does not mean needed - verify how much the first render reads; split rarely-read data behind dynamic import',
      );
    } else {
      lead('clear', 'large modules executed at paint', 'none at current thresholds');
    }

    const groups = siblingVariantGroups(coverage.modules ?? []);
    if (groups.length) {
      lead(
        'open',
        `sibling variant groups (${groups.length})`,
        groups.map((g) => `${g.dir} ${g.files} modules ${kb(g.bytes)}`).join(', '),
        'keep the default variant in the entry, load the active one dynamically',
      );
    } else {
      lead('clear', 'sibling variant groups', 'none detected');
    }
  }

  // Static retained imports: only exists on rolldown builds - omitted entirely
  // elsewhere, UNKNOWN (with the one config line that enables it) when the app
  // detectably builds with rolldown but the graph was never collected.
  const mg = moduleGraphStatus(target, builtAtMs);
  if (mg.state === 'present') {
    const rows = retainedLeadRows(mg.graph);
    if (rows.length) {
      const mods = mg.graph.modules;
      lead(
        'open',
        `statically retained imports from build entries (${rows.length} module(s) >=${kb(GRAPH_RETAINED_OPEN_BYTES)})`,
        rows
          .slice(0, 3)
          .map(
            (m) =>
              `${m.id} retains ${kb(m.retainedBytes)}/${m.retainedModuleCount} module(s)${m.idom != null ? ` via ${mods[m.idom].id}` : ''}`,
          )
          .join(', '),
        `${CLI} what-if <module> prices the deferral (exact modules+bytes freed; --keep a,b holds needed parts eager); ${CLI} cut <module> lists the fewest import edits when several paths reach it; make the importer use dynamic import(). Retained is potential, not proof - if the first render genuinely needs it, justify with a measurement or constraint`,
      );
    } else {
      lead(
        'clear',
        'statically retained imports (build entries only)',
        `no non-framework module retains >=${kb(GRAPH_RETAINED_OPEN_BYTES)} behind a cuttable static edge`,
      );
    }
  } else if (mg.state === 'stale') {
    lead(
      'unknown',
      'static module graph',
      'module-graph.json predates the current build - a build without the devtools flag leaves it stale',
      'keep build.rolldownOptions.devtools = { mode: "report" } in the vite config, rebuild, re-scan',
    );
  } else if (mg.state === 'absent-rolldown') {
    lead(
      'unknown',
      'static module graph',
      'not collected - this app builds with rolldown, so one config line enables static split-candidate ranking',
      `vite >= 8: add build.rolldownOptions.devtools = { mode: "report" } to the vite config, rebuild, re-scan - \`${CLI} graph\` then ranks every candidate by the bytes a deferral frees, \`${CLI} what-if\` prices one cut, no browser run needed`,
    );
  }

  console.log(`verdict for ${target.dist} (entry ${entry})\n`);
  for (const line of lines) console.log(line);
  console.log('');
  if (fresh(runtime) && runtime.earlyStopped) {
    console.log(
      `NOTE: the latest measurement EARLY-STOPPED after run 1 - its LCP delta was >=${runtime.earlyStopped.factor}x the`,
    );
    console.log(
      'noise threshold. This is a probe. Use scan --no-early-stop to check both FCP and LCP before a decision.\n',
    );
  } else if (fresh(runtime) && runtime.runs < 3) {
    console.log(
      'NOTE: the latest measurement used fewer than 3 runs. Its deltas are indicative only.',
    );
    console.log(
      'Check both FCP and LCP with scan --no-early-stop (at least 3 runs) before a keep/revert decision.\n',
    );
  }
  if (openCount + unknownCount + pendingCount === 0) {
    console.log(
      'VERDICT: every signal class is clear and fresh. Nothing further is indicated by these tools.',
    );
    console.log(
      'Boundary: coverage attributes the entry + same-origin pre-paint chunks with sourcemaps;',
    );
    console.log(
      'cross-origin scripts are unattributed, and non-JS weight (fonts/images/CSS) is counted',
    );
    console.log(
      'by type but not analyzed further. Server latency, cache policy, and interaction-time',
    );
    console.log(
      'cost are out of scope. Remaining LCP is baseline network + parse + render for what the',
    );
    console.log('page genuinely needs at first paint.');
    console.log(
      'Rendered output: compared with the original on every pinned page (first screen + full page',
    );
    console.log(
      'pixels, text, accessible names, runtime errors, LCP element) as served statically - states',
    );
    console.log('behind interactions or live API data are not covered.');
  } else {
    console.log(
      `VERDICT: not done - ${openCount} lead(s) OPEN, ${unknownCount} signal(s) UNKNOWN or stale` +
        `${pendingCount ? `, ${pendingCount} check(s) PENDING until you finish` : ''}.`,
    );
    if (openCount === 0 && unknownCount === 0) {
      console.log(
        `Only the final rendered-output comparison is left: \`${CLI} scan --parity\` once you have`,
      );
      console.log('finished optimizing; report only after it is clear.');
    }
    if (parity.state === 'open') {
      console.log(
        'RECOVER MODE: this build renders differently from the original, so none of its LCP numbers',
      );
      console.log(
        `count until it matches again. Fix or revert the change that broke it FIRST (\`${CLI} parity recover\`).`,
      );
    }
    console.log(
      'Work the OPEN items (render gap first), gather the UNKNOWN signals, rebuild, re-measure.',
    );
    console.log('');
    console.log(
      'Do NOT report this work as finished or "confirmed by the harness" while leads are OPEN -',
    );
    console.log('a re-pinned baseline records your gain; it does not close the checklist above.');
    console.log(
      'Copy the checklist verbatim into your final summary. If you stop now, your summary must',
    );
    console.log(
      `say "stopping with ${openCount} lead(s) OPEN" and justify each one with a measurement`,
    );
    console.log(
      '(you tried it and the delta was sub-noise) or a concrete constraint (framework dep,',
    );
    console.log('the first paint genuinely needs it, outside the declared scope).');
  }
}

// --- commands ---------------------------------------------------------------

async function cmdGen(argv) {
  const opts = parse(argv, { force: { type: 'boolean', default: false } });
  const result = generateApp(APP_DIR, { force: opts.force });
  if (!result.written) {
    console.log(`app unchanged: ${result.reason}`);
    return;
  }
  console.log(`demo app generated at ${APP_DIR}`);
  console.log(`features: ${FEATURE_NAMES.map((n) => `${n} (~${FEATURES[n].kb}KB)`).join(', ')}`);
}

async function cmdBuild() {
  if (!fs.existsSync(path.join(APP_DIR, 'src', 'main.ts'))) {
    throw new Error('no app yet - run `node harness.mjs gen` first');
  }
  guardStateDir();
  const result = await buildApp({ appDir: APP_DIR });
  console.log(`built in ${result.wallMs}ms`);
  console.log(`entry main.js: ${kb(result.entryBytes)}`);
  for (const chunk of result.chunks) console.log(`async ${chunk.file}: ${kb(chunk.bytes)}`);
  const m = result.buildMetrics?.metrics;
  if (m) {
    console.log(
      `build metrics: total ${m['build.total_ms']}ms, output ${kb(m['output.total_bytes'] ?? 0)}, ` +
        `max initial load ${kb(m['output.max_initial_load_bytes'] ?? 0)}`,
    );
    console.log(`build report: ${path.join(BUILD_METRICS_DIR, 'metrics.json')}`);
  } else {
    console.log('build metrics report missing (devtools report mode did not run?)');
  }
}

async function cmdMeasure(argv) {
  const opts = parse(argv, {
    ...TARGET_OPTS,
    runs: { type: 'string', default: '5' },
    warmup: { type: 'string', default: '1' },
    label: { type: 'string', default: '' },
    settle: { type: 'string', default: '1500' },
    'no-throttle': { type: 'boolean', default: false },
    features: { type: 'string' },
    pin: { type: 'boolean', default: false },
    'no-early-stop': { type: 'boolean', default: false },
    'net-scale': { type: 'string' },
    ...NETWORK_OPTS,
  });
  const networkFlag = parseNetworkFlags(opts);
  const target = resolveTarget(opts);
  const expectedFeatures = expectedFeaturesFor(target, opts);
  ensureAnchor(target);
  const baselineState = readJson(target.paths.runtimeBaseline);
  const pinnedLcp = baselineState?.metrics?.['runtime.lcp_ms'];
  // The early stop compares run 1 against the pinned baseline - only valid when
  // both were measured under the SAME pinned throttle.
  const expectedId = expectedThrottleId(target, opts, networkFlag);
  const scaleMatches =
    expectedId != null && baselineState != null && throttleId(baselineState) === expectedId;
  const earlyStop =
    !opts.pin && !opts['no-early-stop'] && typeof pinnedLcp === 'number' && scaleMatches
      ? {
          baselineLcp: pinnedLcp,
          thresholdMs: Math.max(NOISE_FLOOR_MS, (NOISE_FLOOR_PCT / 100) * pinnedLcp),
          factor: EARLY_STOP_FACTOR,
        }
      : null;
  const {
    samples,
    earlyStopped,
    throttle: usedThrottle,
  } = await withServerAndBrowser(
    target,
    opts['no-throttle'],
    async ({ origin, cdp, throttle }) => ({
      ...(await gatherSamples(cdp, origin, {
        throttle,
        expectedFeatures,
        runs: Number(opts.runs),
        warmup: Number(opts.warmup),
        settleMs: Number(opts.settle),
        earlyStop,
      })),
      throttle,
    }),
    { netScaleFlag: opts['net-scale'] ?? null, networkFlag },
  );
  const { report, hadBaseline } = writeMeasureReport(target, samples, {
    expectedFeatures,
    label: opts.label,
    throttle: usedThrottle,
    earlyStopped,
  });
  printMeasureSummary(report, hadBaseline, {
    parity: parityStatus(target),
    lcpCheck: noteLcpElement(target, report),
  });
  if (opts.pin) pinBaseline(target);
  else if (!hadBaseline)
    console.log(
      `no pinned baseline yet - run \`${CLI} baseline\` (or pass --pin) to pin this measurement`,
    );
  console.log(`\nfull report: ${target.paths.runtimeMetrics}`);
}

async function cmdCoverage(argv) {
  const opts = parse(argv, {
    ...TARGET_OPTS,
    settle: { type: 'string', default: '2000' },
    'no-throttle': { type: 'boolean', default: false },
    features: { type: 'string' },
    entry: { type: 'string' },
  });
  const target = resolveTarget(opts);
  const expectedFeatures = expectedFeaturesFor(target, opts);
  const entry = requireEntry(target, opts);
  if (!opts.entry) console.log(`entry: ${entry} (auto-detected from dist/index.html)`);
  if (!fs.existsSync(path.join(target.dist, `${entry}.map`))) {
    throw new Error(
      `entry ${entry} has no sourcemap (${path.join(target.dist, entry)}.map) - build with sourcemap: true`,
    );
  }
  const cov = await withServerAndBrowser(target, opts['no-throttle'], ({ origin, cdp, throttle }) =>
    coverageRun(cdp, {
      origin,
      throttle,
      expectedFeatures,
      entryName: `/${entry.replaceAll('\\', '/')}`,
      settleMs: Number(opts.settle),
    }),
  );
  const report = buildCoverageReport(target, cov, entry);
  printCoverageReport(target, report);
  console.log(`\nfull report: ${target.paths.coverage}`);
}

async function cmdProfile(argv) {
  const opts = parse(argv, {
    ...TARGET_OPTS,
    'no-throttle': { type: 'boolean', default: false },
    entry: { type: 'string' },
  });
  const target = resolveTarget(opts);
  const entry = requireEntry(target, opts);
  if (!opts.entry) console.log(`entry: ${entry} (auto-detected from dist/index.html)`);
  const profile = await withServerAndBrowser(
    target,
    opts['no-throttle'],
    ({ origin, cdp, throttle }) => profileRun(cdp, { origin, throttle }),
  );
  const report = buildProfileReport(target, profile, entry);
  printProfileReport(report);
  console.log(`\nfull report: ${target.paths.profile}`);
}

// Everything in one browser session: timed runs, coverage, boot profile — then
// the fused verdict. One command per iteration instead of four.
async function cmdScan(argv) {
  const opts = parse(argv, {
    ...TARGET_OPTS,
    // 3 runs: the median is stable at 3 and each throttled run costs the app's
    // full LCP (minutes on slow pages). --runs 5 remains available for noisy
    // pages; the effect side is regression-gated by the eval ledger.
    runs: { type: 'string', default: '3' },
    warmup: { type: 'string', default: '1' },
    label: { type: 'string', default: '' },
    settle: { type: 'string', default: '1500' },
    'no-throttle': { type: 'boolean', default: false },
    features: { type: 'string' },
    entry: { type: 'string' },
    pin: { type: 'boolean', default: false },
    // One measure run, no profile: a cheap mid-iteration "did my change move
    // LCP" probe on slow apps (a full throttled scan costs minutes when LCP
    // is >10s). Never a basis for accept/revert/pin - verdict says so too.
    quick: { type: 'boolean', default: false },
    // Force the boot-CPU profile leg even when pre-paint CPU is in baseline
    // territory (it is skipped then - the verdict only ever demands a profile
    // for pre-paint CPU >150ms, so skipping never leaves an UNKNOWN).
    profile: { type: 'boolean', default: false },
    // Re-scans print the compact coverage view (top 15 module rows instead of
    // 60 - the actionable sections are never trimmed); --full restores the
    // whole table. First scan of a target is always full.
    full: { type: 'boolean', default: false },
    // Disable the probe's early stop (run 1 stops when its LCP delta vs
    // the pinned baseline is >=5x the noise threshold; see EARLY_STOP_FACTOR).
    'no-early-stop': { type: 'boolean', default: false },
    // Pin the target's net-scale explicitly (one of 1/2/4/8) instead of the
    // calibrated value. Changing it makes the pinned baseline incomparable.
    'net-scale': { type: 'string' },
    // Or pin a network in real units (--bandwidth/--rtt/--cpu), same rules.
    ...NETWORK_OPTS,
    // Rendered-output parity: pages beyond the build's discoverable entries (SPA
    // routes) and masks for regions the page renders differently on every load.
    pages: { type: 'string' },
    mask: { type: 'string', multiple: true },
    // Compare the rendered output with the original in this scan too - the final
    // check once the optimizing is done (recover mode's each-tweak strategy turns
    // it on for every scan).
    parity: { type: 'boolean', default: false },
  });
  if (opts.quick && opts.pin) {
    throw new Error(
      'scan --quick cannot --pin: a 1-run baseline poisons every later delta. Run a full scan to pin.',
    );
  }
  if (opts.quick) {
    opts.runs = '1';
  }
  const networkFlag = parseNetworkFlags(opts);
  const target = resolveTarget(opts);
  const expectedFeatures = expectedFeaturesFor(target, opts);
  const entry = requireEntry(target, opts);
  if (!fs.existsSync(path.join(target.dist, `${entry}.map`))) {
    throw new Error(
      `entry ${entry} has no sourcemap (${path.join(target.dist, entry)}.map) - build with sourcemap: true`,
    );
  }
  // The first scan of a target saves the build as the original - before any change.
  ensureAnchor(target, { pages: parsePages(opts.pages), masks: opts.mask ?? [] });
  // Early stop needs a decision context (a pinned baseline measured under the
  // SAME pinned throttle) and full-median exemptions: --pin measurements
  // BECOME the baseline, so they always sample fully; quick mode is already a
  // single run.
  const baselineState = readJson(target.paths.runtimeBaseline);
  const pinnedLcp = baselineState?.metrics?.['runtime.lcp_ms'];
  const expectedId = expectedThrottleId(target, opts, networkFlag);
  const scaleMatches =
    expectedId != null && baselineState != null && throttleId(baselineState) === expectedId;
  const earlyStop =
    !opts.quick &&
    !opts.pin &&
    !opts['no-early-stop'] &&
    typeof pinnedLcp === 'number' &&
    scaleMatches
      ? {
          baselineLcp: pinnedLcp,
          thresholdMs: Math.max(NOISE_FLOOR_MS, (NOISE_FLOOR_PCT / 100) * pinnedLcp),
          factor: EARLY_STOP_FACTOR,
        }
      : null;

  const gathered = await withServerAndBrowser(
    target,
    opts['no-throttle'],
    async ({ origin, cdp, throttle }) => {
      const { samples, earlyStopped } = await gatherSamples(cdp, origin, {
        throttle,
        expectedFeatures,
        runs: Number(opts.runs),
        warmup: Number(opts.warmup),
        settleMs: Number(opts.settle),
        earlyStop,
      });
      process.stderr.write('coverage run...\n');
      const cov = await coverageRun(cdp, {
        origin,
        throttle,
        expectedFeatures,
        entryName: `/${entry.replaceAll('\\', '/')}`,
        settleMs: 2000,
      });
      let profile = null;
      if (!opts.quick) {
        // The profile only ever matters when pre-paint CPU is above the verdict's
        // 150ms threshold - skip its navigation otherwise (a scan-time minute on
        // slow apps). --profile forces it.
        const prepaintMs = summarize(samples, expectedFeatures).metrics[
          'runtime.prepaint_longtask_ms'
        ];
        if (opts.profile || (typeof prepaintMs === 'number' && prepaintMs > 150)) {
          process.stderr.write('profile run...\n');
          profile = await profileRun(cdp, { origin, throttle });
        } else {
          process.stderr.write(
            `profile skipped (pre-paint CPU ${prepaintMs == null ? 'n/a' : `${Math.round(prepaintMs)}ms`} - baseline territory; force with --profile)\n`,
          );
        }
      }
      // The rendered output is compared once, at the end (scan --parity), not per
      // change - except in recover mode under the each-tweak strategy, which
      // re-checks (and records a snapshot) with every scan.
      const checkParity =
        opts.parity || readJson(target.paths.parity.recover)?.strategy === 'each-tweak';
      const parity = checkParity
        ? await parityLeg(target, {
            cdp,
            throttle,
            currentLcp: summarize(samples, expectedFeatures).lcpElement,
            label: opts.label,
          })
        : null;
      return { samples, earlyStopped, cov, profile, throttle, parity };
    },
    { netScaleFlag: opts['net-scale'] ?? null, networkFlag },
  );

  const { report, hadBaseline } = writeMeasureReport(target, gathered.samples, {
    expectedFeatures,
    label: opts.label,
    throttle: gathered.throttle,
    earlyStopped: gathered.earlyStopped,
  });
  // Read before buildCoverageReport overwrites it: an existing coverage report
  // means this is a re-scan, so the module table prints compact (--full restores it).
  const isRescan = fs.existsSync(target.paths.coverage);
  const coverageReport = buildCoverageReport(target, gathered.cov, entry);
  const profileReport = gathered.profile
    ? buildProfileReport(target, gathered.profile, entry)
    : null;

  // advice:false throughout - the verdict printed below carries every next: line,
  // so the sections above it report numbers without a second copy of the coaching.
  printMeasureSummary(report, hadBaseline, {
    advice: false,
    parity: parityStatus(target),
    lcpCheck: noteLcpElement(target, report),
  });
  console.log('');
  printCoverageReport(target, coverageReport, { compact: isRescan && !opts.full, advice: false });
  console.log('');
  if (profileReport) {
    printProfileReport(profileReport, { advice: false });
    console.log('');
  }
  const graphBuiltAtMs = fs.statSync(path.join(target.dist, entry)).mtimeMs;
  printGraphSection(target, graphBuiltAtMs);
  printEagerDiffSummary(target, graphBuiltAtMs);
  if (gathered.parity) {
    printParityReport(gathered.parity, target);
    console.log('');
  }
  printVerdict(target, { entry });

  if (opts.pin) {
    pinBaseline(target);
  } else if (!hadBaseline) {
    if (opts.quick) {
      console.log(
        '(no pinned baseline yet, and quick scans are never pinned - run a full scan to establish it)',
      );
    } else {
      // First scan of a target IS the baseline: pin it so every later scan
      // reports a baselineDelta without extra ceremony.
      pinBaseline(target);
      console.log('(first scan of this target - pinned as the baseline automatically)');
    }
  }
  console.log(`\nreports: ${target.paths.dir}`);
}

async function cmdVerdict(argv) {
  const opts = parse(argv, { ...TARGET_OPTS, entry: { type: 'string' } });
  printVerdict(resolveTarget(opts), opts);
}

// --- rendered-output parity -------------------------------------------------------
// The numbers above say how much faster the page got; parity says whether it is
// still the same page. A build that renders differently from the original - a style
// that stopped applying, a web font that never loads, a hero swapped for a skeleton -
// can measure faster precisely BECAUSE it broke. So parity gates everything that
// accepts a change: the keep/revert line, pinning, and the verdict's "done".
// See README.md "Rendered-output parity" for the design.

// perf-kit's own state can sit inside the build dir (--dist . with ./.perf-kit):
// never copy, hash or crawl it as part of the app.
const PARITY_EXCLUDE = [STATE_DIR];

const readApprovals = (target) => readJson(target.paths.parity.approvals) ?? { approvals: [] };

/**
 * A human approval turns an OPEN page "approved" - for its exact signature only.
 * A build is only 'clear' once EVERY pinned page has a result; fewer is 'partial'
 * (a recover-mode `parity --only` re-check).
 */
function effectiveParity(report, approvals, pinnedPages) {
  if (!report) return null;
  const pages = report.pages.map((page) => {
    if (page.state !== 'open') return page;
    const approval = approvals.approvals.find(
      (a) => a.page === page.page && a.signature === page.signature,
    );
    return approval ? { ...page, state: 'approved', approval } : page;
  });
  const covered = pinnedPages.every((p) => pages.some((r) => r.page === p));
  const state = pages.some((p) => p.state === 'open')
    ? 'open'
    : !report.integrity || pages.some((p) => p.state === 'unknown')
      ? 'unknown'
      : covered
        ? 'clear'
        : 'partial';
  return { ...report, pages, state };
}

/**
 * Where parity stands for the CURRENT build: 'none' (no original saved), 'identical'
 * (same files as the original), 'stale' (not compared since the last rebuild),
 * 'partial' (some pages compared), or the latest report's state with approvals
 * applied.
 */
function parityStatus(target) {
  const paths = target.paths.parity;
  const manifest = readJson(paths.manifest);
  if (!manifest || !fs.existsSync(path.join(target.dist, 'index.html'))) {
    return { state: 'none', manifest, report: null };
  }
  const currentHash = hashDist(target.dist, { exclude: PARITY_EXCLUDE });
  const stored = readJson(paths.report);
  const report =
    stored?.distHash === currentHash && stored.anchor?.distHash === manifest.distHash
      ? effectiveParity(
          stored,
          readApprovals(target),
          manifest.pages.map((p) => p.path),
        )
      : null;
  if (currentHash === manifest.distHash) return { state: 'identical', manifest, report };
  if (!report) return { state: 'stale', manifest, report: null };
  return { state: report.state, manifest, report };
}

const throttleKey = (throttle) => throttleId(throttle);

/**
 * LCP-element bookkeeping on every measurement - free, the timed runs observed it.
 * On the original build itself it records the original's LCP element; on any other
 * build it compares with it. Shown as information; only the final check gates on it.
 */
function noteLcpElement(target, report) {
  const paths = target.paths.parity;
  const manifest = readJson(paths.manifest);
  if (!manifest || !report.lcpElement) return null;
  const key = throttleKey(report.throttle);
  if (hashDist(target.dist, { exclude: PARITY_EXCLUDE }) === manifest.distHash) {
    // a single run is no majority - quick probes never define the original's
    if (manifest.lcp?.key !== key && report.runs > 1) {
      manifest.lcp = { key, element: report.lcpElement, measuredAtMs: Date.now() };
      writeJson(paths.manifest, manifest);
    }
    return null;
  }
  return manifest.lcp?.key === key ? compareLcp(manifest.lcp.element, report.lcpElement) : null;
}

/** The LCP-element comparison a later measurement of an already-compared build allows. */
function lateLcpCheck(manifest, runtime) {
  if (!runtime.lcpElement || manifest.lcp?.key !== throttleKey(runtime.throttle)) return null;
  return compareLcp(manifest.lcp.element, runtime.lcpElement);
}

/** What a reader of a "clear" must also know: approvals, masks added late, a replaced original. */
function parityExtras(report) {
  if (!report) return '';
  const parts = [];
  const approved = report.pages.filter((p) => p.state === 'approved');
  if (approved.length) {
    parts.push(
      `${approved.length} difference(s) approved by a human: ${approved
        .map((p) => `${p.page} "${p.approval.reason}"`)
        .join(', ')}`,
    );
  }
  if (report.anchor.lateMasks.length) {
    parts.push(`masks added after the original was saved: ${report.anchor.lateMasks.join(', ')}`);
  }
  if (report.anchor.replaced) parts.push(`the original was replaced ${report.anchor.replaced}x`);
  if (report.anchor.late) {
    parts.push('the original was saved after a perf baseline existed - it may include changes');
  }
  return parts.length ? ` (${parts.join('; ')})` : '';
}

function parsePages(value) {
  return value
    ? value
        .split(',')
        .map((p) => p.trim())
        .filter(Boolean)
        .map((p) => normalizePagePath(p))
    : [];
}

/**
 * Save the original build on a target's first scan / measure / parity; afterwards
 * only ADD pages and masks (pages widen coverage; a mask narrows it, so every mask
 * added after the save is listed in each verdict).
 */
function ensureAnchor(target, { pages = [], masks = [] } = {}) {
  const paths = target.paths.parity;
  const manifest = readJson(paths.manifest);
  if (!fs.existsSync(path.join(target.dist, 'index.html'))) return manifest;
  if (!manifest) {
    guardStateDir();
    const discovered = discoverPages(target.dist, { exclude: PARITY_EXCLUDE });
    const late = fs.existsSync(target.paths.runtimeBaseline);
    const created = saveAnchor({
      dist: target.dist,
      paths,
      pages: Array.from(new Set(['/', ...discovered.pages, ...pages])),
      masks,
      git: gitState(path.dirname(target.dist), target.dist),
      late,
      exclude: PARITY_EXCLUDE,
    });
    writeJson(paths.manifest, created);
    console.log(
      `original build saved for rendered-output checks - page(s): ${created.pages.map((p) => p.path).join(' ')}`,
    );
    console.log(
      '  every later build is compared against it; scan --pin / baseline never replace it.',
    );
    if (discovered.more.length) {
      console.log(
        `  ${discovered.more.length} more entry page(s) not pinned (add with --pages): ${discovered.more.slice(0, 5).join(' ')}`,
      );
    }
    if (discovered.skipped.length) {
      console.log(
        `  skipped self-contained HTML (no bundled script/stylesheet): ${discovered.skipped.slice(0, 5).join(' ')}`,
      );
    }
    if (created.pages.length === 1) {
      console.log(
        '  client-side routes are not discoverable from the build: pin the ones that matter with --pages /route,/other',
      );
    }
    if (late) {
      console.log(
        '  note: a perf baseline was pinned before this - this build may already include changes. If it does,',
      );
      console.log(
        `  a human rebuilds the original commit and runs \`${CLI} parity anchor --replace\` on that build.`,
      );
    }
    if (created.git?.dirty) {
      console.log(
        '  note: the app has uncommitted changes - the saved build may already include some of them.',
      );
    }
    return created;
  }
  let changed = false;
  for (const page of pages) {
    if (manifest.pages.some((p) => p.path === page)) continue;
    manifest.pages.push({ path: page, addedAtMs: Date.now() });
    changed = true;
    console.log(`parity: page ${page} added`);
  }
  for (const selector of masks) {
    if (manifest.masks.some((m) => m.selector === selector)) continue;
    manifest.masks.push({ selector, addedAtMs: Date.now() });
    changed = true;
    console.log(
      `parity: mask ${selector} added - masks added after the original was saved are listed in every verdict`,
    );
  }
  if (changed) writeJson(paths.manifest, manifest);
  return manifest;
}

/**
 * The original's LCP element under the given throttle. A full scan of the original
 * itself records it for free (noteLcpElement); otherwise it is measured once here
 * (3 throttled runs against the saved build) and kept - the original never changes.
 */
async function anchorLcpElement(target, manifest, { cdp, throttle }) {
  const key = throttleKey(throttle);
  if (manifest.lcp?.key === key) return manifest.lcp.element;
  const server = await startServer(target.paths.parity.anchorDist);
  let element;
  try {
    const samples = [];
    for (let i = 1; i <= 3; i++) {
      process.stderr.write(`parity: LCP element of the original build, run ${i}/3 (once)...\n`);
      samples.push(await timedRun(cdp, { url: `${server.origin}/`, throttle, settleMs: 1500 }));
    }
    element = lcpElementOf(samples);
  } finally {
    await server.close();
  }
  manifest.lcp = { key, element, measuredAtMs: Date.now() };
  writeJson(target.paths.parity.manifest, manifest);
  return element;
}

/**
 * Compare the current build with the original and write the report - every pinned
 * page, or just `only` (a recover-mode re-check; results for the same build merge,
 * so a build is clear once every page has been compared). `currentLcp` is this
 * build's LCP element from a timed measurement under `throttle`; undefined means
 * there is none to compare (the report says so).
 */
async function parityLeg(target, { cdp, throttle, currentLcp, label = null, only = [] }) {
  const paths = target.paths.parity;
  const manifest = ensureAnchor(target);
  if (!manifest) return null;
  const pinned = manifest.pages.map((p) => p.path);
  const checking = only.length ? only : pinned;
  for (const page of checking) {
    if (!pinned.includes(page)) {
      throw new Error(`${page} is not a pinned page (${pinned.join(' ')}) - add it with --pages`);
    }
  }
  const distHash = hashDist(target.dist, { exclude: PARITY_EXCLUDE });
  const identical = distHash === manifest.distHash;
  const integrity = hashDist(paths.anchorDist) === manifest.distHash;
  let lcp = null;
  if (integrity && !identical && currentLcp !== undefined && checking.includes('/')) {
    lcp = compareLcp(await anchorLcpElement(target, manifest, { cdp, throttle }), currentLcp);
  }
  // Same files as the original renders the same by construction - nothing to capture.
  const results =
    integrity && !identical
      ? await runParity({
          cdp,
          anchorDist: paths.anchorDist,
          candidateDist: target.dist,
          pages: checking,
          masks: manifest.masks.map((m) => m.selector),
          outDir: paths.pages,
          lcpByPage: lcp ? { '/': lcp } : {},
          log: (line) => process.stderr.write(`${line}\n`),
        })
      : [];
  const previous = readJson(paths.report);
  const carried =
    previous?.distHash === distHash && previous.anchor?.distHash === manifest.distHash
      ? previous.pages.filter((p) => !checking.includes(p.page))
      : [];
  const pages = identical
    ? pinned.map((page) => ({ page, state: 'clear', reasons: [], notes: [] }))
    : [...carried, ...results].sort((a, b) => pinned.indexOf(a.page) - pinned.indexOf(b.page));
  const report = {
    schemaVersion: 1,
    generatedAtMs: Date.now(),
    dist: target.dist,
    distHash,
    identical,
    integrity,
    lcpCompared: identical || Boolean(pages.find((p) => p.page === '/')?.lcp),
    anchor: {
      createdAtMs: manifest.createdAtMs,
      distHash: manifest.distHash,
      git: manifest.git,
      late: manifest.late,
      replaced: (manifest.replaced ?? []).length,
      lateMasks: manifest.masks
        .filter((m) => m.addedAtMs > manifest.createdAtMs)
        .map((m) => m.selector),
      lcp: manifest.lcp?.element ?? null,
    },
    pages,
  };
  writeJson(paths.report, report);
  const effective = effectiveParity(report, readApprovals(target), pinned);
  recordCheck(target, effective, {
    label,
    captured: results.map((r) => r.page),
  });
  return effective;
}

/**
 * Every check is recorded - history.jsonl plus a copy of the pages it captured
 * under snapshots/<n>/ - and moves recover mode: a difference enters it; a build
 * compared on every page without one (or identical to the original) leaves it.
 */
function recordCheck(target, report, { label, captured }) {
  const paths = target.paths.parity;
  const n = fs.existsSync(paths.history)
    ? fs.readFileSync(paths.history, 'utf8').split('\n').filter(Boolean).length + 1
    : 1;
  const recover = readJson(paths.recover);
  let event = null;
  if (report.state === 'open' && !recover) {
    writeJson(paths.recover, { enteredAtMs: Date.now(), atCheck: n, strategy: 'on-demand' });
    event = 'entered recover mode';
  } else if (
    recover &&
    report.integrity &&
    (report.identical || report.state === 'clear' || report.state === 'unknown')
  ) {
    fs.rmSync(paths.recover, { force: true });
    event = 'left recover mode';
  }
  for (const page of captured) {
    const from = path.join(paths.pages, pageSlug(page));
    if (fs.existsSync(from)) {
      fs.cpSync(from, path.join(paths.snapshots, String(n), pageSlug(page)), { recursive: true });
    }
  }
  guardStateDir();
  fs.appendFileSync(
    paths.history,
    `${JSON.stringify({
      n,
      label,
      atMs: Date.now(),
      distHash: report.distHash,
      identical: report.identical,
      state: report.state,
      event,
      pages: report.pages
        .filter((p) => captured.includes(p.page))
        .map((p) => ({ page: p.page, state: p.state, reasons: p.reasons })),
    })}\n`,
  );
}

/** What to do once a check found a difference - printed with it, and by `parity recover`. */
function printRecoverGuide(target) {
  const strategy = readJson(target.paths.parity.recover)?.strategy ?? 'on-demand';
  // bisect runs from the app's repo: spell perf-kit by absolute path
  const kit = CLI === 'node harness.mjs' ? `node ${path.join(ROOT, 'harness.mjs')}` : CLI;
  const say = (line = '') => console.log(line);
  say(
    'RECOVER MODE - this build renders differently from the original, so its LCP numbers do not count',
  );
  say('until it matches again.');
  say(
    '  1. Diagnose: each OPEN page names what changed (element + property, rendered font, missing',
  );
  say(
    '     text, new runtime error, LCP element) and points at its diff image (red = changed, blue =',
  );
  say('     varies between loads of the original, not judged).');
  say(`  2. Choose how to re-check your fixes (now: ${strategy}):`);
  say(
    `       ${CLI} parity strategy on-demand    re-check when you decide: \`parity\` (every page), \`parity --only /page\` (the broken ones), --label <tweak> names the snapshot`,
  );
  say(`       ${CLI} parity strategy each-tweak   every scan re-checks and records a snapshot`);
  say('  3. Not sure which kept change broke it? Bisect your commits - the check exits 1 while a');
  say('     page differs:');
  say(`       git bisect run sh -c '<build command> && ${kit} parity --only <page> --exit-code'`);
  say(`  4. ${CLI} parity history   every check of this recovery, with its snapshot images`);
  say(
    '  A fix may give back part of the gain: the number you report must come from a build that matches',
  );
  say(
    '  the original. Recover mode ends when a check covers every page and finds no difference - never by',
  );
  say('  approving, replacing the original or adding masks (human decisions).');
}

function describeRegionElement(e) {
  const name = e.text ? `${e.label} "${e.text.slice(0, 30)}"` : e.label;
  if (e.gone) return `${name} is gone`;
  if (e.added) return `${name} is new`;
  if (e.movedIn) return `${name} moved here`;
  const bits = [];
  if (e.font) bits.push(`rendered in ${e.font.to} (was ${e.font.from})`);
  for (const c of e.changes ?? []) bits.push(`${c.prop} ${c.from} -> ${c.to}`);
  if (e.moved) {
    const r = (b) => `${b.x},${b.y} ${b.w}x${b.h}`;
    bits.push(`moved ${r(e.moved.from)} -> ${r(e.moved.to)}`);
  }
  return bits.length ? `${name}: ${bits.join(', ')}` : name;
}

function printParityDetails(page) {
  const out = (line) => console.log(`      ${line}`);
  const list = (label, items, n = 5) => {
    if (!items.length) return;
    const more = items.length > n ? ` (+${items.length - n} more)` : '';
    out(
      `${label}: ${items
        .slice(0, n)
        .map((s) => JSON.stringify(s))
        .join(', ')}${more}`,
    );
  };
  for (const [name, view] of Object.entries(page.views)) {
    for (const region of view.regions.slice(0, 3)) {
      const where = name === 'viewport' ? 'first screen' : 'full page';
      const what = region.elements?.length
        ? `: ${region.elements.map((e) => describeRegionElement(e)).join('; ')}`
        : '';
      out(`${where} region ${region.w}x${region.h} at ${region.x},${region.y}${what}`);
    }
  }
  list('text missing', page.text.missing);
  list('text new', page.text.added);
  list('accessible names missing', page.a11y.missing);
  list('accessible names new', page.a11y.added);
  for (const error of page.runtime.added.slice(0, 5)) out(`new runtime error: ${error}`);
  for (const f of page.fonts.slice(0, 3)) {
    out(
      `font: ${f.label}${f.text ? ` "${f.text.slice(0, 30)}"` : ''} rendered in ${f.to} (was ${f.from})`,
    );
  }
  list('web fonts no longer loaded', page.faces.missing, 4);
  list('images missing', page.images.missing, 3);
  list('images new', page.images.added, 3);
  const styles = page.styles;
  if (styles?.changedCount) {
    const examples = styles.changed
      .slice(0, 2)
      .map(
        (d) => `${d.label} ${d.changes.map((c) => `${c.prop} ${c.from} -> ${c.to}`).join(', ')}`,
      );
    out(`computed style changed on ${styles.changedCount} element(s), e.g. ${examples.join('; ')}`);
  }
  if (styles?.missingCount) {
    out(`elements gone: ${styles.missingCount} (${styles.missing.slice(0, 3).join(', ')})`);
  }
  if (styles?.addedCount) {
    out(`elements new: ${styles.addedCount} (${styles.added.slice(0, 3).join(', ')})`);
  }
  if (styles?.movedCount) out(`${styles.movedCount} element(s) moved or resized`);
}

function printParityReport(report, target) {
  if (!report) return;
  const a = report.anchor;
  const saved = new Date(a.createdAtMs).toISOString().slice(0, 16).replace('T', ' ');
  const commit = a.git ? `, commit ${a.git.sha}${a.git.dirty ? ' + uncommitted changes' : ''}` : '';
  console.log(`rendered output vs the original build (saved ${saved} UTC${commit}):`);
  if (!report.integrity) {
    console.log(
      '  the saved original build was modified after it was saved - nothing can be compared against it',
    );
    return;
  }
  if (report.identical) {
    console.log('  this build is byte-identical to the original - same rendering by construction');
    return;
  }
  for (const page of report.pages) {
    const tag = { open: 'OPEN', unknown: 'UNKNOWN', approved: 'approved', clear: 'clear' }[
      page.state
    ];
    console.log(
      `  ${page.page}  ${tag}${page.reasons.length ? ` - ${page.reasons.join('; ')}` : ''}`,
    );
    if (page.approval) {
      console.log(
        `      approved by ${page.approval.by} ${new Date(page.approval.approvedAtMs).toISOString().slice(0, 10)}: "${page.approval.reason}"`,
      );
    }
    if (page.state === 'open') printParityDetails(page);
    for (const note of page.notes) console.log(`      note: ${note}`);
    const evidence = Object.entries(page.files ?? {}).filter(([name]) => name.startsWith('diff'));
    if (page.state !== 'clear' && evidence.length) {
      console.log(`      evidence: ${evidence.map(([, file]) => file).join('  ')}`);
    }
  }
  const root = report.pages.find((p) => p.page === '/');
  if (!report.lcpCompared) {
    console.log(
      '  LCP element: not compared - no fresh timed measurement of this build (scan --parity compares it)',
    );
  } else if (root?.lcp?.state === 'same') {
    console.log(`  LCP element: same as the original - ${describeLcp(root.lcp.current)}`);
  }
  if (report.state === 'partial') {
    const pinned = readJson(target.paths.parity.manifest)?.pages.length ?? report.pages.length;
    console.log(
      `  ${report.pages.length} of ${pinned} page(s) compared for this build so far - it is clear once every pinned page is`,
    );
  }
  if (a.lateMasks.length) {
    console.log(`  masks added after the original was saved: ${a.lateMasks.join(', ')}`);
  }
  if (a.replaced) {
    console.log(`  the original was replaced ${a.replaced}x since it was first saved`);
  }
  if (report.state === 'open') {
    console.log('');
    printRecoverGuide(target);
  }
}

async function cmdParity(argv) {
  const sub = {
    approve: cmdParityApprove,
    anchor: cmdParityAnchor,
    strategy: cmdParityStrategy,
    history: cmdParityHistory,
    recover: cmdParityRecover,
  }[argv[0]];
  if (sub) return sub(argv.slice(1));
  const opts = parse(argv, {
    ...TARGET_OPTS,
    pages: { type: 'string' },
    mask: { type: 'string', multiple: true },
    only: { type: 'string' },
    label: { type: 'string' },
    'exit-code': { type: 'boolean', default: false },
  });
  const target = resolveTarget(opts);
  const manifest = ensureAnchor(target, { pages: parsePages(opts.pages), masks: opts.mask ?? [] });
  if (!manifest) {
    throw new Error(
      `no build at ${target.dist} - build the app first.\n` +
        '(Wrong target? --app <appRoot> resolves dist/build/out; --dist <builtDir> aims directly.)',
    );
  }
  // The LCP element is only compared against a fresh timed measurement of THIS
  // build (a scan's); without one, no throttled run is spent here.
  const entry = detectEntry(target.dist);
  const builtAtMs = entry ? fs.statSync(path.join(target.dist, entry)).mtimeMs : Infinity;
  const runtime = readJson(target.paths.runtimeMetrics);
  const fresh = runtime && runtime.entry === entry && runtime.generatedAtMs >= builtAtMs;
  const report = await withServerAndBrowser(target, true, ({ cdp }) =>
    parityLeg(target, {
      cdp,
      throttle: fresh ? runtime.throttle : null,
      currentLcp: fresh ? (runtime.lcpElement ?? null) : undefined,
      label: opts.label ?? null,
      only: parsePages(opts.only),
    }),
  );
  console.log('');
  printParityReport(report, target);
  const tag = { open: 'OPEN', unknown: 'UNKNOWN', partial: 'partial', clear: 'clear' }[
    report.state
  ];
  console.log(`\nparity: ${tag} - full report ${target.paths.parity.report}`);
  // lets `git bisect run` find the change that broke the page
  if (opts['exit-code'] && report.state === 'open') process.exitCode = 1;
}

async function cmdParityStrategy(argv) {
  const { values: opts, positionals } = parseArgs({
    args: argv,
    options: TARGET_OPTS,
    allowPositionals: true,
  });
  const target = resolveTarget(opts);
  const recover = readJson(target.paths.parity.recover);
  if (!recover) {
    console.log(
      `not in recover mode - the rendered output is compared once, when you finish (${CLI} scan --parity); a strategy applies after that finds a difference`,
    );
    return;
  }
  const choice = positionals[0];
  if (!choice) {
    console.log(`recover strategy: ${recover.strategy}`);
    return;
  }
  if (choice !== 'each-tweak' && choice !== 'on-demand') {
    throw new Error('usage: parity strategy each-tweak|on-demand');
  }
  recover.strategy = choice;
  writeJson(target.paths.parity.recover, recover);
  console.log(
    choice === 'each-tweak'
      ? 'each-tweak: every scan now re-checks the rendered output and records a snapshot, until recover mode ends'
      : `on-demand: re-check when you decide - \`${CLI} parity\` (every page) or \`${CLI} parity --only <page>\``,
  );
}

async function cmdParityRecover(argv) {
  const target = resolveTarget(parse(argv, { ...TARGET_OPTS }));
  if (!readJson(target.paths.parity.recover)) {
    console.log(
      `not in recover mode - nothing found to differ. The comparison runs once, when you finish: ${CLI} scan --parity`,
    );
    return;
  }
  printRecoverGuide(target);
}

async function cmdParityHistory(argv) {
  const target = resolveTarget(parse(argv, { ...TARGET_OPTS }));
  const paths = target.paths.parity;
  const checks = fs.existsSync(paths.history)
    ? fs
        .readFileSync(paths.history, 'utf8')
        .split('\n')
        .filter(Boolean)
        .map((line) => JSON.parse(line))
    : [];
  if (!checks.length) {
    console.log('no rendered-output checks recorded for this target yet');
    return;
  }
  for (const check of checks) {
    const when = new Date(check.atMs).toISOString().slice(0, 16).replace('T', ' ');
    const what = check.identical ? 'identical to the original' : check.state;
    console.log(
      `#${check.n}${check.label ? ` "${check.label}"` : ''}  ${when} UTC  build ${check.distHash.slice(0, 8)}  ${what}${check.event ? `  (${check.event})` : ''}`,
    );
    for (const page of check.pages) {
      console.log(
        `    ${page.page}  ${page.state}${page.reasons.length ? ` - ${page.reasons.join('; ')}` : ''}`,
      );
    }
    if (check.pages.length) {
      console.log(`    snapshot: ${path.join(paths.snapshots, String(check.n))}`);
    }
  }
  const recover = readJson(paths.recover);
  console.log(
    recover
      ? `\nin recover mode since check #${recover.atCheck} (strategy: ${recover.strategy})`
      : '\nnot in recover mode',
  );
}

async function cmdParityApprove(argv) {
  const { values: opts, positionals } = parseArgs({
    args: argv,
    options: { ...TARGET_OPTS, reason: { type: 'string' } },
    allowPositionals: true,
  });
  const pagePath = positionals[0];
  if (!pagePath || !opts.reason?.trim()) {
    throw new Error(
      'usage: parity approve <page> --reason "<why this difference is intended>"\n' +
        'Approving is a HUMAN decision about a difference they looked at - an agent must never run it.',
    );
  }
  const target = resolveTarget(opts);
  const status = parityStatus(target);
  if (!status.report) {
    throw new Error(
      `no current parity check for this build - run \`${CLI} parity\`, look at the evidence, then approve`,
    );
  }
  const page = status.report.pages.find((p) => p.page === pagePath);
  if (!page) {
    throw new Error(
      `no page ${pagePath} in the last check (pages: ${status.report.pages.map((p) => p.page).join(' ')})`,
    );
  }
  if (page.state !== 'open') throw new Error(`${pagePath} is ${page.state} - nothing to approve`);
  const approvals = readApprovals(target);
  approvals.approvals.push({
    page: pagePath,
    signature: page.signature,
    reasons: page.reasons,
    reason: opts.reason.trim(),
    by: os.userInfo().username,
    approvedAtMs: Date.now(),
  });
  writeJson(target.paths.parity.approvals, approvals);
  console.log(`approved ${pagePath}: ${page.reasons.join('; ')}`);
  console.log(
    'it covers this exact difference only - any further change to the page reopens it. Every verdict lists it.',
  );
}

async function cmdParityAnchor(argv) {
  const opts = parse(argv, { ...TARGET_OPTS, replace: { type: 'boolean', default: false } });
  const target = resolveTarget(opts);
  const paths = target.paths.parity;
  const manifest = readJson(paths.manifest);
  if (!manifest) {
    console.log(
      `no original build saved for ${target.dist} - \`${CLI} parity\` (or a scan) saves the current one`,
    );
    return;
  }
  if (opts.replace) {
    if (!fs.existsSync(path.join(target.dist, 'index.html'))) {
      throw new Error(
        `no build at ${target.dist} - build the commit that should become the original`,
      );
    }
    const next = saveAnchor({
      dist: target.dist,
      paths,
      pages: [],
      git: gitState(path.dirname(target.dist), target.dist),
      late: false,
      exclude: PARITY_EXCLUDE,
    });
    // Pages, masks (with their dates) and the first-save time carry over: lateness
    // is measured from the FIRST save, and a replacement is recorded, never hidden.
    next.createdAtMs = manifest.createdAtMs;
    next.pages = manifest.pages;
    next.masks = manifest.masks;
    next.replaced = [
      ...(manifest.replaced ?? []),
      { atMs: Date.now(), previousHash: manifest.distHash, previousGit: manifest.git },
    ];
    writeJson(paths.manifest, next);
    fs.rmSync(paths.report, { force: true });
    fs.rmSync(paths.approvals, { force: true });
    console.log(
      `the saved original build was REPLACED by ${target.dist}${next.git ? ` (commit ${next.git.sha})` : ''}.`,
    );
    console.log(
      'Parity now compares against it, and every verdict says the original was replaced. This is a human decision -',
    );
    console.log('an agent must never replace the original to make a difference go away.');
    return;
  }
  const date = (msValue) => new Date(msValue).toISOString().slice(0, 16).replace('T', ' ');
  console.log(`original build for ${target.dist}:`);
  console.log(`  saved ${date(manifest.createdAtMs)} UTC from ${manifest.dist}`);
  if (manifest.git) {
    console.log(
      `  commit ${manifest.git.sha}${manifest.git.dirty ? ' + uncommitted changes' : ''}`,
    );
  }
  console.log(`  files hash ${manifest.distHash.slice(0, 12)} (copy at ${paths.anchorDist})`);
  for (const page of manifest.pages) {
    const later = page.addedAtMs > manifest.createdAtMs ? ` (added ${date(page.addedAtMs)})` : '';
    console.log(`  page ${page.path}${later}`);
  }
  for (const mask of manifest.masks) {
    const later = mask.addedAtMs > manifest.createdAtMs ? ` (added ${date(mask.addedAtMs)})` : '';
    console.log(`  mask ${mask.selector}${later}`);
  }
  if (manifest.lcp?.element) {
    console.log(`  LCP element (${manifest.lcp.key}): ${describeLcp(manifest.lcp.element)}`);
  }
  for (const r of manifest.replaced ?? []) {
    console.log(`  replaced ${date(r.atMs)} (previous hash ${r.previousHash.slice(0, 12)})`);
  }
  if (manifest.late) {
    console.log('  saved after a perf baseline existed - it may already include changes');
  }
}

// --- module-graph analysis (rolldown devtools report builds) ----------------------

// Scan/verdict integration thresholds: a non-framework module retaining >=100KB on
// the initial load is always worth pricing with what-if; the 30s slack absorbs the
// moments between the graph write (generate stage) and the dist flush of one build.
const GRAPH_RETAINED_OPEN_BYTES = 100 * 1024;
const GRAPH_FRESH_SLACK_MS = 30 * 1000;
const GRAPH_FRAMEWORK_RE = /^(react-dom|react|scheduler|vue|@vue|svelte|preact)(\/|$)/;

function moduleGraphStatus(target, builtAtMs = null) {
  const graph = loadModuleGraph(
    moduleGraphCandidates({
      demoMetricsDir: target.isDemo ? BUILD_METRICS_DIR : null,
      dist: target.dist,
    }),
  );
  if (graph) {
    const stale =
      typeof builtAtMs === 'number' &&
      fs.statSync(graph.file).mtimeMs < builtAtMs - GRAPH_FRESH_SLACK_MS;
    return { state: stale ? 'stale' : 'present', graph };
  }
  const rolldownBuilt = target.isDemo || rolldownBuildDetected(path.dirname(target.dist));
  return { state: rolldownBuilt ? 'absent-rolldown' : 'absent' };
}

// Evidence that rolldown actually bundles this app - a planted node_modules/rolldown
// (the link: launcher vehicle) proves nothing. Real evidence: the app declares
// rolldown as its own dependency, or the vite it resolves is rolldown-powered
// (vite >= 8, or the rolldown-vite alias). Walks up for hoisted monorepo installs.
function rolldownBuildDetected(appRoot) {
  let viteSpec = null;
  let dir = appRoot;
  for (let i = 0; i < 4; i++) {
    const pkg = readJson(path.join(dir, 'package.json'));
    const deps = { ...pkg?.dependencies, ...pkg?.devDependencies };
    if (deps.rolldown) return true;
    if (deps.vite && !viteSpec) viteSpec = deps.vite;
    const vitePkg = readJson(path.join(dir, 'node_modules', 'vite', 'package.json'));
    if (vitePkg) {
      return (
        vitePkg.name === 'rolldown-vite' || Number(String(vitePkg.version ?? '').split('.')[0]) >= 8
      );
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  if (viteSpec) {
    return /rolldown-vite/.test(viteSpec) || Number((viteSpec.match(/\d+/) ?? [0])[0]) >= 8;
  }
  return false;
}

// Boot roots (a script dominated directly by an HTML entry) are excluded: deferring
// the module the HTML loads is a blank page, and their children surface in the
// ranking anyway. Children of a JS entry are ordinary candidates.
function retainedLeadRows(graph) {
  const entrySet = new Set(graph.entryModules);
  const mods = graph.modules;
  const isHtmlBootChild = (m) =>
    m.idom != null && entrySet.has(mods[m.idom].id) && /\.html?$/i.test(mods[m.idom].id);
  return mods
    .filter(
      (m) =>
        m.staticReachable &&
        !entrySet.has(m.id) &&
        m.retainedBytes >= GRAPH_RETAINED_OPEN_BYTES &&
        !isHtmlBootChild(m) &&
        !GRAPH_FRAMEWORK_RE.test(m.id),
    )
    .sort((a, b) => b.retainedBytes - a.retainedBytes || a.id.localeCompare(b.id));
}

// Group note (P1a adoption): when >=2 printed rows come from the same package (a
// node_modules package, or the same top-level source subdir), a combined deferral can
// free shared internals no single row's retained size counts. Ids are slash-normalized
// first (Windows). `packageRoot` returns the grouping key for one id.
function packageRoot(id) {
  const norm = id.replaceAll('\\', '/');
  const nm = norm.lastIndexOf('node_modules/');
  if (nm >= 0) {
    const rest = norm.slice(nm + 'node_modules/'.length).split('/');
    return rest[0]?.startsWith('@') && rest[1] ? `${rest[0]}/${rest[1]}` : rest[0];
  }
  const segs = norm.split('/');
  return segs.length >= 2 ? `${segs[0]}/${segs[1]}` : segs[0];
}

// The single line printed when one package root repeats across the display rows (the
// most-repeated one wins), or null. Teaches what-if's group pricing from a printed
// surface (adoption rule 3) without opening a verdict lead (rule 2).
function packageRootNote(rows) {
  const counts = new Map();
  for (const m of rows) {
    const root = packageRoot(m.id);
    counts.set(root, (counts.get(root) ?? 0) + 1);
  }
  let best = null;
  for (const [root, n] of counts) {
    if (n >= 2 && (!best || n > best.n)) best = { root, n };
  }
  if (!best) return null;
  return `note: ${best.root} appears in ${best.n} rows - \`${CLI} what-if <a> <b>\` prices the combined deferral; shared internals can make it exceed the sum.`;
}

function printGraphSection(target, builtAtMs = null) {
  const mg = moduleGraphStatus(target, builtAtMs);
  if (mg.state === 'absent') return;
  if (mg.state === 'stale') {
    console.log(
      'static module graph: STALE (predates the current build) - rebuild with build.rolldownOptions.devtools = { mode: "report" } in the vite config\n',
    );
    return;
  }
  if (mg.state === 'absent-rolldown') {
    console.log(
      `static module graph: not collected - this app builds with rolldown. vite >= 8: add build.rolldownOptions.devtools = { mode: "report" } to the vite config, rebuild, re-scan (unlocks \`${CLI} graph\` / \`${CLI} what-if\`)\n`,
    );
    return;
  }
  const mods = mg.graph.modules;
  const entrySet = new Set(mg.graph.entryModules);
  const rows = mods
    .filter((m) => m.staticReachable && m.retainedBytes > 0 && !entrySet.has(m.id))
    .sort((a, b) => b.retainedBytes - a.retainedBytes || a.id.localeCompare(b.id))
    .slice(0, 8);
  if (!rows.length) return;
  console.log(
    'statically retained imports (rolldown module graph - what one deferral would free):',
  );
  for (const mod of rows) {
    console.log(
      `  ${kb(mod.retainedBytes).padStart(10)}  ${mod.id}${mod.idom != null ? `  via ${mods[mod.idom].id}` : ''}`,
    );
  }
  const note = packageRootNote(rows);
  if (note) console.log(`  ${note}`);
  console.log(
    `  full ranking: \`${CLI} graph\`; exact modules+bytes for one cut: \`${CLI} what-if <module>\`\n`,
  );
}

// The full graph-diff render (the `graph-diff` command). Eager-tier diff is the headline;
// node classification (added/removed/bytes/edges) follows. Sections are sorted |Δ| desc.
function printGraphDiff(diff) {
  const line = (sign, m) => `  ${`${sign}${kb(m.bytes)}`.padStart(11)}  ${m.id}`;
  console.log(
    `eager set: +${diff.entered.length} module(s)/+${kb(diff.enteredBytes)} entered, ` +
      `-${diff.left.length}/-${kb(diff.leftBytes)} left the initial load\n`,
  );
  if (diff.entered.length) {
    console.log('entered the initial load (weight the change ADDED to first paint):');
    for (const m of diff.entered.slice(0, 15)) console.log(line('+', m));
    if (diff.entered.length > 15) console.log(`  ... +${diff.entered.length - 15} more`);
    printDiffGroups(diff.enteredGroups, '+');
    console.log('');
  }
  if (diff.left.length) {
    console.log('left the initial load (weight the change DEFERRED off first paint):');
    for (const m of diff.left.slice(0, 15)) console.log(line('-', m));
    if (diff.left.length > 15) console.log(`  ... +${diff.left.length - 15} more`);
    printDiffGroups(diff.leftGroups, '-');
    console.log('');
  }
  if (!diff.entered.length && !diff.left.length) {
    console.log('eager set unchanged - the initial load holds exactly the same modules.');
    console.log('(an unchanged eager set does not promise an unchanged measurement: CSS, assets,');
    console.log(' ordering and cache behavior still move it.)\n');
  }
  const nodes = [];
  if (diff.added.length) nodes.push(`+${diff.added.length} added`);
  if (diff.removed.length) nodes.push(`-${diff.removed.length} removed`);
  if (diff.bytesChanged.length) nodes.push(`${diff.bytesChanged.length} bytes-changed`);
  if (diff.edgesChanged.length) nodes.push(`${diff.edgesChanged.length} edges-changed`);
  console.log(
    `nodes: ${nodes.length ? nodes.join(', ') : 'no id-level changes'} (renames read as add+remove - ids are identity).`,
  );
}

// Exclusion-aware rollup: bytes attributed to a changed subtree root count only nodes
// that themselves changed; untouched intermediates are a skip count, not a listing.
function printDiffGroups(groups, sign) {
  const multi = groups.filter((g) => g.count > 1 || g.skipped > 0);
  if (!multi.length) return;
  console.log('  grouped under nearest changed ancestor:');
  for (const g of multi.slice(0, 8)) {
    console.log(
      `    ${sign}${kb(g.bytes)} across ${g.count} changed module(s) under ${g.rootId}` +
        `${g.skipped ? ` (+${g.skipped} unchanged dep(s) skipped)` : ''}`,
    );
  }
}

// Scan integration (rule 3, fact-only): when a pinned baseline graph exists and the
// current sidecar is fresh, print <=3 lines. Never tells the agent to skip a measure.
function printEagerDiffSummary(target, builtAtMs) {
  const baseline = loadModuleGraph([target.paths.baselineModuleGraph]);
  if (!baseline) return;
  const mg = moduleGraphStatus(target, builtAtMs);
  if (mg.state !== 'present') return;
  const diff = diffGraphs(baseline, mg.graph);
  if (!diff.entered.length && !diff.left.length) {
    console.log(
      'eager set vs pinned baseline: unchanged (CSS/assets/order can still move the measurement).\n',
    );
    return;
  }
  const topStr = diff.entered.length
    ? `top entered ${diff.entered[0].id} (+${kb(diff.entered[0].bytes)})`
    : `top left ${diff.left[0].id} (-${kb(diff.left[0].bytes)})`;
  console.log(
    `eager set vs pinned baseline: +${diff.entered.length} module(s)/+${kb(diff.enteredBytes)} entered, ` +
      `-${diff.left.length}/-${kb(diff.leftBytes)} left; ${topStr}.`,
  );
  console.log(
    `(deterministic eager-set attribution; \`${CLI} graph-diff\` shows the full account.)\n`,
  );
}

function requireModuleGraph(opts) {
  // A remembered Worker target answers the static queries too (its report sits in a
  // per-environment directory); a browser target is only resolved when none applies.
  const workerCandidates = opts.report ? null : workerReportCandidates(opts);
  let candidates;
  let target = null;
  let appHint = null; // where the app's rolldown resolves from, for the missing-report cause
  if (workerCandidates) {
    candidates = workerCandidates;
    appHint = readJson(WORKER_TARGET_FILE)?.app ?? null;
  } else {
    target = resolveTarget(opts);
    candidates = moduleGraphCandidates({
      reportDir: opts.report,
      demoMetricsDir: target.isDemo ? BUILD_METRICS_DIR : null,
      dist: target.dist,
    });
    appHint = target.isDemo ? null : path.dirname(target.dist);
  }
  const graph = loadModuleGraph(candidates);
  if (!graph) {
    const cause = appHint && !opts.report ? reportSupportNote(appHint, CLI) : null;
    throw new Error(
      (cause ? `${cause}\n\n` : '') +
        'no module-graph.json found - it is written by rolldown devtools report builds.\n' +
        'vite >= 8: add `build.rolldownOptions.devtools = { mode: "report" }` to the vite config\n' +
        '(report lands in node_modules/.rolldown/report/<output dir>), rebuild, then re-run.\n' +
        `For a Cloudflare Worker: \`${CLI} worker-config --app <appDir>\` writes a config that puts the report\n` +
        'on every environment. Or point at an existing report with --report <dir>.',
    );
  }
  if (target && !opts.report) assertReportMatchesDist(graph, target.dist);
  return graph;
}

// A report perf-kit discovered for the dist must describe that dist. Seen in the field:
// an agent ranked a server build's graph against the client dist, because the SSR build
// ran last and overwrote the one shared report directory. Report mode now writes one
// directory per output dir, but a rebuild without the report still leaves a stale one.
// An explicit --report is the user's choice and is not checked.
function assertReportMatchesDist(graph, dist) {
  const why = reportDistMismatch(graph, dist);
  if (!why) return;
  throw new Error(
    `${why}\n` +
      'Rebuild with the report on, so the report and the dist come from the same build.\n' +
      'Report mode writes one report per output dir: builds that share an outDir share a report.\n' +
      'Or point at the client report with --report <dir>.',
  );
}

async function cmdGraph(argv) {
  const opts = parse(argv, {
    ...TARGET_OPTS,
    report: { type: 'string' },
    top: { type: 'string', default: '15' },
  });
  const graph = requireModuleGraph(opts);
  const mods = graph.modules;
  console.log(`module graph: ${graph.file}`);
  console.log(`entries: ${graph.entryModules.join(', ')}`);
  const staticMods = mods.filter((m) => m.staticReachable);
  const staticBytes = staticMods.reduce((sum, m) => sum + m.bytes, 0);
  const lazyCount = mods.filter((m) => m.dynamicOnly).length;
  console.log(
    `initial-load view: ${staticMods.length} modules / ${kb(staticBytes)} statically reachable; ${lazyCount} already lazy (dynamic-import only)\n`,
  );
  const entrySet = new Set(graph.entryModules);
  const rows = staticMods
    .filter((m) => m.retainedBytes > 0 && !entrySet.has(m.id))
    .sort((a, b) => b.retainedBytes - a.retainedBytes || a.id.localeCompare(b.id))
    .slice(0, Number(opts.top));
  console.log(
    "retained size - what deferring each module's import edge would remove from the initial load:",
  );
  for (const mod of rows) {
    const via = mod.idom != null ? `  via ${mods[mod.idom].id}` : '  (directly under the entries)';
    console.log(
      `  ${kb(mod.retainedBytes).padStart(10)}  ${mod.id}  (own ${kb(mod.bytes)}, ${mod.retainedModuleCount} module(s))${via}`,
    );
  }
  if (!rows.length)
    console.log(
      '  (nothing sizeable is uniquely retained - the entries themselves hold the bytes)',
    );
  const note = packageRootNote(rows);
  if (note) console.log(`\n${note}`);
  console.log(
    `\nnext: ${CLI} what-if <module>  - the exact modules+bytes that one deferral frees (add --keep a,b to hold some imports eager)`,
  );
}

async function cmdWhatIf(argv) {
  const { values: opts, positionals } = parseArgs({
    args: argv,
    options: {
      ...TARGET_OPTS,
      report: { type: 'string' },
      keep: { type: 'string' },
      root: { type: 'string' },
    },
    allowPositionals: true,
  });
  if (!positionals.length)
    throw new Error(
      `usage: ${CLI} what-if <module> [<module> ...] [--keep a,b] [--root a,b] [--report <dir>]`,
    );
  const built = requireModuleGraph(opts);
  const resolve = (q) => {
    const hit = resolveModule(built, q);
    if (!hit)
      throw new Error(`no module matches '${q}' (ids are project-relative, e.g. src/router.ts)`);
    if (hit.ambiguous) {
      throw new Error(
        `'${q}' is ambiguous:\n  ${hit.ambiguous.join('\n  ')}\nuse a longer suffix.`,
      );
    }
    return hit.index;
  };
  const resolved = positionals.map(resolve);
  const keep = (opts.keep ?? '').split(',').filter(Boolean).map(resolve);
  // `--root`: modules that load on every start without a static import from an entry
  // (see withRoots) - the initial load priced below is the entries + their closure.
  const graph = withRoots(built, (opts.root ?? '').split(',').filter(Boolean).map(resolve));
  printRootedLoad(graph);
  // Entries cannot be deferred - they ARE the initial load (no import edge to cut).
  // Filtered up front so single and group queries answer the same way.
  const entrySet = new Set(graph.entryModules);
  const rootSet = new Set(graph.rootModules ?? []);
  const targets = [];
  for (const t of resolved) {
    if (rootSet.has(graph.modules[t].id)) {
      console.log(
        `${graph.modules[t].id} is a --root - it stays eager with its closure, so there is no import edge to defer. Skipped.`,
      );
    } else if (entrySet.has(graph.modules[t].id)) {
      console.log(
        `${graph.modules[t].id} is an entry - it IS the initial load, so there is no import edge to defer. Skipped; \`${CLI} graph\` ranks its heavy children instead.`,
      );
    } else {
      targets.push(t);
    }
  }
  if (!targets.length) return;
  if (targets.length > 1) {
    printCombinedWhatIf(graph, targets, keep);
    return;
  }
  const target = targets[0];
  const result = whatIf(graph, target, keep);

  console.log(`what-if deferred: ${result.target.id}`);
  if (result.notStaticallyReachable) {
    console.log(
      result.alreadyLazy
        ? `already lazy: every path to this module crosses a dynamic import - it costs the initial load nothing.${rootSet.size ? '' : ROOT_HINT}`
        : 'not reachable from the entries at all in this build.',
    );
    return;
  }
  if (result.cutEdges.length) {
    console.log(
      `cut ${result.cutEdges.length} static import edge(s), from: ${result.cutEdges.join(', ')}`,
    );
  }
  console.log(
    `removes ${kb(result.removedBytes)} / ${result.removedCount} module(s) from the initial load${keep.length ? ` (keeping ${keep.length} sentry module(s) eager)` : ''}:`,
  );
  const shown = result.removed.slice(0, 20);
  for (const mod of shown) {
    console.log(`  ${kb(mod.bytes).padStart(10)}  ${mod.id}`);
  }
  if (result.removed.length > shown.length) {
    console.log(`  ... +${result.removed.length - shown.length} more`);
  }
  // Adoption fact (rule 3): when a single upstream cut detaches the module with FEWER
  // edits than deferring every direct import, point at `cut`. Only prints when it wins,
  // so the common (naive-is-minimal) case stays byte-identical.
  const cut = minCut(graph, target, keepProtectedEdges(graph, keep));
  if (
    !cut.blockedByProtected &&
    !cut.hasUncuttableSink &&
    cut.flow > 0 &&
    cut.flow < result.cutEdges.length
  ) {
    console.log(
      `\nmin cut: ${cut.flow} edge(s) instead of ${result.cutEdges.length} - \`${CLI} cut ${result.target.id}\` lists them.`,
    );
  }
  console.log(
    `\nnext: make those importer(s) load it with a dynamic import(), rebuild, run the app's functional check, then \`${CLI} scan\` to confirm the LCP effect.`,
  );
}

// Every static import edge into a `--keep` sentry is protected: the min cut must not
// recommend deferring an import the user is holding eager, so it routes around them.
function keepProtectedEdges(graph, keep) {
  const edges = [];
  for (const s of keep) for (const p of graph.staticPreds[s]) edges.push({ from: p, to: s });
  return edges;
}

// "Already lazy" in the build can still be startup code: an app whose boot chain is a
// row of dynamic imports (`import('./initReact')`) pays for them on every load.
const ROOT_HINT =
  ' If the app runs that dynamic import on every start, pass the module it loads as --root to price it as startup code.';

// With --root, the "initial load" every line below prices is the startup load: the
// static closure of the entries + the roots. Say so once, with its size.
function printRootedLoad(graph) {
  if (!graph.rootModules?.length) return;
  const base = evalOverrides(graph, []);
  console.log(`roots: ${graph.rootModules.join(', ')}`);
  console.log(
    `"initial load" below = the static closure of the entries + ${graph.rootModules.length} root(s): ${kb(base.eagerBytes)} / ${base.eager.size} module(s)\n`,
  );
}

// Multiple targets = ONE combined deferral, priced with a single overlay. The payoff is
// the comparison against the summed-individually total: shared internals dominated by no
// single target only leave the initial load when the whole group is deferred, so the
// combined free can exceed the sum (the firebase / sibling-routes pattern).
function printCombinedWhatIf(graph, targets, keep) {
  const keepRoots = keep.filter((k) => !targets.includes(k));
  const overrides = targets.flatMap((t) => deferAllInto(graph, t));
  const combined = evalOverrides(graph, overrides, keepRoots.length ? keepRoots : null);
  const summed = targets.reduce((sum, t) => sum + whatIf(graph, t, keep).removedBytes, 0);
  const ids = targets.map((t) => graph.modules[t].id);
  console.log(`what-if deferred (combined): ${ids.join(', ')}`);
  console.log(
    `removes ${kb(combined.removedBytes)} / ${combined.removedCount} module(s) from the initial load` +
      `${keepRoots.length ? ` (keeping ${keepRoots.length} sentry module(s) eager)` : ''}:`,
  );
  const shown = combined.removed.slice(0, 20);
  for (const mod of shown) console.log(`  ${kb(mod.bytes).padStart(10)}  ${mod.id}`);
  if (combined.removed.length > shown.length)
    console.log(`  ... +${combined.removed.length - shown.length} more`);
  const delta = combined.removedBytes - summed;
  const together = targets.length === 2 ? 'both' : 'all';
  if (delta > 0) {
    console.log(
      `\ncombined ${kb(combined.removedBytes)} vs ${kb(summed)} summed individually` +
        ` - ${kb(delta)} of shared internals count only when ${together} are deferred.`,
    );
  } else if (delta < 0) {
    console.log(
      `\ncombined ${kb(combined.removedBytes)} vs ${kb(summed)} summed individually` +
        " - the targets' retained sets overlap, so the group frees less than the naive sum.",
    );
  } else {
    console.log(
      `\ncombined ${kb(combined.removedBytes)} vs ${kb(summed)} summed individually` +
        ' - no shared internals; the targets are independent, so the combined free equals the sum.',
    );
  }
  console.log(
    `\nnext: make each importer load its target with a dynamic import(), rebuild, run the app's functional check, then \`${CLI} scan\` to confirm the LCP effect.`,
  );
}

async function cmdCut(argv) {
  const { values: opts, positionals } = parseArgs({
    args: argv,
    options: {
      ...TARGET_OPTS,
      report: { type: 'string' },
      keep: { type: 'string' },
      root: { type: 'string' },
    },
    allowPositionals: true,
  });
  const query = positionals[0];
  if (!query) {
    throw new Error(`usage: ${CLI} cut <module> [--keep a,b] [--root a,b] [--report <dir>]`);
  }
  const graph = requireModuleGraph(opts);
  const resolve = (q) => {
    const hit = resolveModule(graph, q);
    if (!hit)
      throw new Error(`no module matches '${q}' (ids are project-relative, e.g. src/router.ts)`);
    if (hit.ambiguous) {
      throw new Error(
        `'${q}' is ambiguous:\n  ${hit.ambiguous.join('\n  ')}\nuse a longer suffix.`,
      );
    }
    return hit.index;
  };
  const target = resolve(query);
  const keep = (opts.keep ?? '').split(',').filter(Boolean).map(resolve);
  // `--root`: modules whose whole closure must stay eager because the runtime loads them
  // without a static import from an entry - a Durable Object or Workflow class the
  // entry exports and the runtime loads by name, or a dynamic import the app runs on
  // every start. They join the entries (withRoots), so the cut lands on the edges BELOW
  // them, and a module only a root reaches becomes cuttable. `--keep` cannot say this:
  // protecting a root's in-edges only pushes the cut up to the entry.
  const roots = (opts.root ?? '').split(',').filter(Boolean).map(resolve);
  const rooted = withRoots(graph, roots);
  const mod = rooted.modules[target];
  printRootedLoad(rooted);

  // Trivial cases reuse what-if's flags: a module that costs the initial load nothing
  // has nothing to cut.
  if (mod.staticReachable === false) {
    console.log(`min cut for ${mod.id}`);
    console.log(
      mod.dynamicOnly === true
        ? `already lazy: every path to this module crosses a dynamic import - nothing to cut.${roots.length ? '' : ROOT_HINT}`
        : 'not reachable from the entries at all in this build - nothing to cut.',
    );
    return;
  }
  const cut = minCut(rooted, target, keepProtectedEdges(rooted, keep));
  if (cut.hasUncuttableSink) {
    console.log(`min cut for ${mod.id}`);
    console.log(
      roots.includes(target)
        ? 'this module is a --root - it stays eager with its closure; drop it from --root to cut it.'
        : 'this module is an entry - it cannot be detached by cutting imports (you would delete the module itself).',
    );
    return;
  }
  if (cut.blockedByProtected) {
    console.log(`min cut for ${mod.id}`);
    console.log(
      'every path to it runs through a --keep-protected import - no cut avoids them. Relax --keep to cut it.',
    );
    return;
  }
  if (cut.flow === 0 || !cut.cutEdges.length) {
    console.log(`min cut for ${mod.id}`);
    console.log('nothing to cut - it is already detached from the initial load.');
    return;
  }
  const naive = whatIf(rooted, target, keep).cutEdges.length;
  const priced = evalOverrides(
    rooted,
    cut.cutEdges.map((e) => ({ ...e, kind: 'defer' })),
  );
  console.log(
    `min cut for ${mod.id} - ${cut.flow} import edge(s) to make dynamic (naive what-if would touch ${naive}):`,
  );
  for (const e of cut.cutEdges) {
    console.log(`  ${rooted.modules[e.from].id}  ->  ${rooted.modules[e.to].id}`);
  }
  console.log(
    `make each import dynamic (import()) -> frees ${kb(priced.removedBytes)} / ${priced.removedCount} module(s) from the initial load` +
      `${keep.length ? ` (keeping ${keep.length} sentry module(s) eager)` : ''}` +
      `${roots.length ? ` (${roots.length} root module(s) stay eager with their closure)` : ''}.`,
  );
  console.log(
    `\nnext: open those file(s), change the static import to a dynamic import(), rebuild, run the app's functional check, then \`${CLI} scan\` to confirm the LCP effect.`,
  );
}

async function cmdGraphDiff(argv) {
  const opts = parse(argv, {
    ...TARGET_OPTS,
    report: { type: 'string' },
    against: { type: 'string' },
  });
  const target = resolveTarget(opts);
  const current = loadModuleGraph(
    moduleGraphCandidates({
      reportDir: opts.report,
      demoMetricsDir: target.isDemo ? BUILD_METRICS_DIR : null,
      dist: target.dist,
    }),
  );
  if (!current) {
    const cause =
      target.isDemo || opts.report ? null : reportSupportNote(path.dirname(target.dist), CLI);
    throw new Error(
      (cause ? `${cause}\n\n` : '') +
        'no current module-graph.json found - it is written by rolldown devtools report builds.\n' +
        'vite >= 8: add `build.rolldownOptions.devtools = { mode: "report" }` to the vite config,\n' +
        'rebuild, then re-run. Or point at an existing report with --report <dir>.',
    );
  }
  if (!opts.report) assertReportMatchesDist(current, target.dist);
  // Default: compare against the pinned baseline snapshot; --against compares vs any saved
  // sidecar (a dir or a module-graph.json path). Absent baseline exits normally (no error).
  const baseline = opts.against
    ? loadModuleGraph(moduleGraphCandidates({ reportDir: opts.against }))
    : loadModuleGraph([target.paths.baselineModuleGraph]);
  if (!baseline) {
    console.log(
      opts.against
        ? `no module graph found at --against ${opts.against}`
        : `no pinned baseline module graph yet - pin one with \`${CLI} scan --pin\` or \`${CLI} baseline\`, or pass --against <file>.`,
    );
    return;
  }
  console.log(
    `graph-diff: ${opts.against ? 'against' : 'pinned baseline'} ${baseline.file}\n         -> current ${current.file}\n`,
  );
  printGraphDiff(diffGraphs(baseline, current));
}

async function cmdBaseline(argv) {
  const opts = parse(argv, { ...TARGET_OPTS });
  pinBaseline(resolveTarget(opts));
}

async function cmdTarget(argv) {
  const { values: opts, positionals: positional } = parseArgs({
    args: argv,
    options: {
      demo: { type: 'boolean', default: false },
      'net-scale': { type: 'string' },
      ...NETWORK_OPTS,
    },
    allowPositionals: true,
  });
  const networkFlag = parseNetworkFlags(opts);
  if (opts.demo) {
    fs.rmSync(TARGET_FILE, { force: true });
    console.log('target cleared - commands now default to the demo app');
    return;
  }
  if (positional[0]) {
    const dist = resolveAppDist(positional[0]);
    writeJson(TARGET_FILE, { dist, setAtMs: Date.now() });
    console.log(`target: ${dist}`);
  } else {
    const sticky = readJson(TARGET_FILE);
    console.log(
      sticky?.dist
        ? `target: ${sticky.dist}`
        : 'no target set - commands default to the demo app (set one: node harness.mjs target <appDir>)',
    );
    if (!sticky?.dist && !opts['net-scale'] && !networkFlag) return;
  }
  // Pin the network without a probe run - the operator declares the network a
  // deployed environment measured; every later scan/measure inherits it.
  if (networkFlag) {
    const target = resolveTarget({});
    const existing = readJson(target.paths.throttleProfile);
    const id = throttleId({ network: networkFlag });
    writeJson(target.paths.throttleProfile, {
      schemaVersion: 1,
      network: networkFlag,
      source: 'target-cmd',
      pinnedAtMs: Date.now(),
    });
    console.log(
      `throttle profile pinned: ${id}${existing && throttleId(existing) !== id ? ` (was ${throttleId(existing)} - any pinned baseline is now incomparable, re-pin with scan --pin)` : ''}`,
    );
  }
  // Pin the net-scale without a probe run - the operator (or eval prep) declares
  // it; every later scan/measure and the scoring legs inherit it.
  if (opts['net-scale'] != null) {
    const netScale = Number(opts['net-scale']);
    if (!CALIBRATE_LADDER.includes(netScale)) {
      throw new Error(
        `--net-scale must be one of ${CALIBRATE_LADDER.join('/')} (named, reproducible steps only)`,
      );
    }
    const target = resolveTarget({});
    const existing = readJson(target.paths.throttleProfile);
    writeJson(target.paths.throttleProfile, {
      schemaVersion: 1,
      netScale,
      source: 'target-cmd',
      pinnedAtMs: Date.now(),
    });
    console.log(
      `throttle profile pinned: net x${netScale}${existing && throttleId(existing) !== throttleId({ netScale }) ? ` (was ${throttleId(existing)} - any pinned baseline is now incomparable, re-pin with scan --pin)` : ''}`,
    );
  }
}

async function cmdDefer(argv, mode) {
  const feature = argv[0];
  if (!feature)
    throw new Error(
      `usage: node harness.mjs ${mode === 'deferred' ? 'defer' : 'undefer'} <${FEATURE_NAMES.join('|')}>`,
    );
  const result = setFeatureMode(APP_DIR, feature, mode);
  console.log(`${feature}: ${mode}${result.changed ? '' : ' (already)'}`);
  if (result.changed) console.log('rebuild before measuring: node harness.mjs build');
}

async function cmdStatus() {
  const sticky = readJson(TARGET_FILE);
  console.log(sticky?.dist ? `target: ${sticky.dist}` : 'target: (none - demo app)');
  const target = resolveTarget({});
  const entryName = detectEntry(target.dist);
  console.log(
    `entry chunk: ${
      entryName && fs.existsSync(path.join(target.dist, entryName))
        ? `${entryName} ${kb(fs.statSync(path.join(target.dist, entryName)).size)}`
        : '(not built)'
    }`,
  );
  const last = readJson(target.paths.runtimeState);
  if (last)
    console.log(
      `last measure: ${last.label ?? '(unlabeled)'} LCP ${ms(last.metrics['runtime.lcp_ms'])}`,
    );
  const baseline = readJson(target.paths.runtimeBaseline);
  console.log(
    baseline
      ? `pinned baseline: ${baseline.label ?? '(unlabeled)'} LCP ${ms(baseline.metrics['runtime.lcp_ms'])}`
      : 'pinned baseline: none',
  );
  const parity = parityStatus(target);
  console.log(
    parity.manifest
      ? `original build: saved ${new Date(parity.manifest.createdAtMs).toISOString().slice(0, 16).replace('T', ' ')} UTC, ${parity.manifest.pages.length} page(s) - rendered output ${parity.state}`
      : 'original build: not saved yet (the first scan/measure/parity saves it)',
  );
  if (target.isDemo) {
    const modes = featureModes(APP_DIR);
    if (modes) {
      console.log('feature modes:');
      for (const [name, mode] of Object.entries(modes)) console.log(`  ${name.padEnd(12)} ${mode}`);
    }
  }
}

async function cmdServe(argv) {
  const opts = parse(argv, { ...TARGET_OPTS, port: { type: 'string', default: '4646' } });
  const target = resolveTarget(opts);
  const server = await startServer(target.dist, Number(opts.port));
  console.log(`serving ${target.dist} at ${server.origin} (Ctrl+C to stop)`);
  await new Promise(() => {});
}

// --- dispatch ----------------------------------------------------------------

// --- server-runtime startup mode ---------------------------------------------------
// The browser half of this harness measures a page load; this half measures a process
// coming up. They share the question — "what does the initial load pay for, and what
// can leave it?" — and therefore share the whole static half: `graph`, `what-if`, `cut`
// and `graph-diff` price a deferral identically for a server bundle, because a
// top-level import and a render-blocking import are the same edge in the module graph.
// Only the measurement differs, and it differs completely: no navigation, no paint, no
// transfer, so no throttle (see lib/node/measure.mjs).

const NODE_TARGET_FILE = path.join(STATE_DIR, 'node-target.json');
const NODE_CACHE_MODES = new Set(['cold', 'warm', 'off']);

const NODE_TARGET_OPTS = {
  entry: { type: 'string' },
  ready: { type: 'string' },
  args: { type: 'string' },
  exec: { type: 'string' },
  cache: { type: 'string' },
  cwd: { type: 'string' },
};

function nodeTargetPaths(entry) {
  const dir = path.join(STATE_DIR, 'node', targetSlug(entry));
  return {
    dir,
    metrics: path.join(dir, 'startup-metrics.json'),
    state: path.join(dir, '.state.json'),
    baseline: path.join(dir, 'baseline.json'),
    history: path.join(dir, 'history.jsonl'),
  };
}

/**
 * Resolve (and remember) the server target. Readiness is part of the target, not of a
 * single invocation: a scan that measured "port 3000 accepting" and a later one that
 * measured "process exited" are not comparable, so the spec is pinned with the entry
 * and reused by every later bare command.
 */
function resolveNodeTarget(opts) {
  const sticky = readJson(NODE_TARGET_FILE);
  const entryArg = opts.entry ?? sticky?.entry;
  if (!entryArg) {
    throw new Error(
      'no server target yet - name the BUILT entry script (the file you deploy) and how it\n' +
        'signals readiness, e.g.\n' +
        `  ${CLI} node-scan --entry dist/server.js --ready port:3000\n` +
        `  ${CLI} node-scan --entry dist/cli.js --ready exit`,
    );
  }
  const entry = path.resolve(entryArg);
  if (!fs.existsSync(entry)) {
    throw new Error(
      `no such entry script: ${entry}\n` +
        'Build the app first, then aim --entry at the BUILT file. Never change the build to fit\n' +
        'this tool - aim the tool at the build instead.',
    );
  }
  const readySpec = opts.ready ?? sticky?.ready ?? 'exit';
  const cache = opts.cache ?? sticky?.cache ?? 'cold';
  if (!NODE_CACHE_MODES.has(cache)) {
    throw new Error(`--cache must be one of cold|warm|off (got ${cache}).`);
  }
  const target = {
    entry,
    dist: path.dirname(entry),
    readySpec,
    ready: parseReadySpec(readySpec),
    args: (opts.args ?? sticky?.args ?? '').split(' ').filter(Boolean),
    exec: opts.exec ?? sticky?.exec ?? process.execPath,
    cwd: path.resolve(opts.cwd ?? sticky?.cwd ?? path.dirname(entry)),
    cache,
    paths: nodeTargetPaths(entry),
  };
  const changed =
    sticky?.entry !== entry ||
    sticky?.ready !== readySpec ||
    sticky?.cache !== cache ||
    sticky?.exec !== target.exec ||
    sticky?.cwd !== target.cwd;
  if (changed) {
    writeJson(NODE_TARGET_FILE, {
      entry,
      ready: readySpec,
      args: target.args.join(' '),
      exec: target.exec,
      cwd: target.cwd,
      cache,
      setAtMs: Date.now(),
    });
    console.log(`target: ${entry} (ready: ${describeReady(target.ready)}) - remembered`);
  }
  return target;
}

/**
 * Run options for run `index`. In cold mode every run gets its own empty compile-cache
 * dir, so each pays full parse+compile the way a fresh container does; warm mode shares
 * one dir that the warmup run primes.
 */
function nodeRunOptsFactory(target, { timeoutMs = 60_000 } = {}) {
  const slug = targetSlug(target.entry);
  const shared = target.cache === 'warm' ? freshCacheDir(STATE_DIR, `${slug}-warm`) : null;
  return (index) => ({
    execPath: target.exec,
    entry: target.entry,
    args: target.args,
    cwd: target.cwd,
    ready: target.ready,
    timeoutMs,
    env: runEnv({
      cacheMode: target.cache,
      cacheDir: target.cache === 'cold' ? freshCacheDir(STATE_DIR, `${slug}-${index}`) : shared,
    }),
  });
}

function nodeEntrySources(target) {
  const mapFile = `${target.entry}.map`;
  if (!fs.existsSync(mapFile)) return null;
  try {
    return {
      code: fs.readFileSync(target.entry, 'utf8'),
      map: JSON.parse(fs.readFileSync(mapFile, 'utf8')),
    };
  } catch {
    return null;
  }
}

function nodeModuleGraph(opts, target) {
  return loadModuleGraph(
    moduleGraphCandidates({ reportDir: opts.report, demoMetricsDir: null, dist: target.dist }),
  );
}

async function cmdNodeMeasure(argv) {
  const opts = parse(argv, {
    ...NODE_TARGET_OPTS,
    // Startup runs cost milliseconds, not the minutes a throttled navigation costs,
    // so the default sample count is higher than the browser side's.
    runs: { type: 'string', default: '7' },
    warmup: { type: 'string', default: '1' },
    label: { type: 'string', default: '' },
    pin: { type: 'boolean', default: false },
    timeout: { type: 'string', default: '60' },
  });
  const target = resolveNodeTarget(opts);
  await assertReadyPortFree(target.ready);
  const summary = await runNodeTiming(target, {
    runs: Number(opts.runs),
    warmup: Number(opts.warmup),
    timeoutMs: Number(opts.timeout) * 1000,
  });
  const { report, hadBaseline } = writeNodeReport(target, summary, { label: opts.label });
  printNodeSummary(report, hadBaseline);
  if (opts.pin) pinNodeBaseline(target);
  console.log(`\nfull report: ${target.paths.metrics}`);
}

async function runNodeTiming(target, { runs, warmup, timeoutMs }) {
  const runOptsFor = nodeRunOptsFactory(target, { timeoutMs });
  process.stderr.write(`boot floor (empty script on this runtime)...\n`);
  const floor = await measureBootFloor({ execPath: target.exec });
  const samples = await gatherStartupSamples(runOptsFor, {
    runs,
    warmup,
    onSample: (i, run) =>
      process.stderr.write(`run ${i + 1}/${runs}: ready in ${ms(run.elapsedMs)}\n`),
  });
  return summarizeStartup(samples, {
    bootFloorMs: floor.length ? floor.sort((a, b) => a - b)[Math.floor(floor.length / 2)] : null,
    ready: describeReady(target.ready),
    // Only the polled probes quantize; stdout and exit are observed on the event itself.
    resolutionMs: target.ready.kind === 'port' || target.ready.kind === 'http' ? POLL_MS : 0,
  });
}

function writeNodeReport(target, summary, { label, coverage = null, profile = null }) {
  const prev = readJson(target.paths.state);
  const baseline = readJson(target.paths.baseline);
  const report = {
    schemaVersion: 1,
    generatedAtMs: Date.now(),
    label: label || null,
    entry: target.entry,
    ready: describeReady(target.ready),
    cache: target.cache,
    runs: summary.runs,
    metrics: summary.metrics,
    guard: summary.guard,
    coverage,
    profile,
    delta: prev ? deltaSection(prev.metrics, summary.metrics) : null,
    baselineDelta: baseline ? deltaSection(baseline.metrics, summary.metrics) : null,
    samples: summary.samples,
  };
  writeJson(target.paths.metrics, report);
  writeJson(target.paths.state, {
    schemaVersion: 1,
    tsMs: report.generatedAtMs,
    label: report.label,
    runs: report.runs,
    ready: report.ready,
    cache: report.cache,
    metrics: report.metrics,
  });
  fs.appendFileSync(
    target.paths.history,
    `${JSON.stringify({
      tsMs: report.generatedAtMs,
      label: report.label,
      metrics: report.metrics,
      guard: report.guard,
    })}\n`,
  );
  return { report, hadBaseline: Boolean(baseline) };
}

function pinNodeBaseline(target) {
  const state = readJson(target.paths.state);
  if (!state) throw new Error('nothing to pin - run node-scan first');
  if (state.runs < 3) {
    throw new Error(
      `refusing to pin a ${state.runs}-run measurement - a thin median poisons every later delta. Re-run node-scan with the default sample count.`,
    );
  }
  writeJson(target.paths.baseline, state);
  console.log(`baseline pinned (startup ${ms(state.metrics['runtime.startup_ms'])})`);
}

function printNodeSummary(report, hadBaseline) {
  const m = report.metrics;
  console.log(
    `\n${report.label ? `[${report.label}] ` : ''}${report.runs} runs, cache ${report.cache}`,
  );
  console.log(
    `startup ${ms(m['runtime.startup_ms'])} (p75 ${ms(m['runtime.startup_p75_ms'])}) | ` +
      `runtime boot floor ${ms(m['runtime.boot_floor_ms'])} | ` +
      `app ${ms(m['runtime.app_startup_ms'])} | ready: ${report.ready}`,
  );
  const spread = report.guard.spreadPct;
  console.log(
    `guard: ${report.guard.allRunsCompleted ? 'PASS' : 'FAIL (a run never became ready)'}` +
      `${spread > 25 ? ` - WARNING spread ${spread}% across runs: something outside the bundle moves this number, deltas are not trustworthy` : ''}`,
  );
  const limited = report.guard.resolutionLimited;
  if (limited) {
    console.log(
      `  note: ${round(limited.appMs)}ms of addressable startup is only a few ${limited.resolutionMs}ms probe intervals wide -` +
        ' changes smaller than a few ms cannot be resolved by a readiness poll. Judge byte/module counts here, not milliseconds.',
    );
  }
  const base = report.baselineDelta?.['runtime.app_startup_ms'];
  if (base) {
    const floor = Math.max(NODE_NOISE_FLOOR_MS, (NODE_NOISE_FLOOR_PCT / 100) * base.prev);
    const call =
      Math.abs(base.delta) < floor
        ? `within noise (floor ${Math.round(floor)}ms) - no effect proven`
        : base.delta < 0
          ? 'IMPROVEMENT beyond noise'
          : 'REGRESSION beyond noise';
    console.log(
      `vs pinned baseline: app startup ${base.delta >= 0 ? '+' : ''}${Math.round(base.delta)}ms (${base.pct}%) - ${call}`,
    );
  } else if (!hadBaseline) {
    console.log('no pinned baseline yet - pass --pin to make this the fixed reference');
  }
}

// Everything in one go: timed runs, then the two instrumented runs that say where the
// time went, then the leads. One command per iteration.
async function cmdNodeScan(argv) {
  const opts = parse(argv, {
    ...NODE_TARGET_OPTS,
    report: { type: 'string' },
    runs: { type: 'string', default: '7' },
    warmup: { type: 'string', default: '1' },
    label: { type: 'string', default: '' },
    pin: { type: 'boolean', default: false },
    top: { type: 'string', default: '10' },
    timeout: { type: 'string', default: '60' },
  });
  const target = resolveNodeTarget(opts);
  await assertReadyPortFree(target.ready);
  const timeoutMs = Number(opts.timeout) * 1000;
  const summary = await runNodeTiming(target, {
    runs: Number(opts.runs),
    warmup: Number(opts.warmup),
    timeoutMs,
  });

  const runOptsFor = nodeRunOptsFactory(target, { timeoutMs });
  process.stderr.write('coverage run (what had to execute to become ready)...\n');
  const cov = await startupCoverage(runOptsFor('cov'), { root: target.dist });

  const sources = nodeEntrySources(target);
  let profile = null;
  if (sources) {
    process.stderr.write('profile run (where the pre-ready time went)...\n');
    const raw = await startupProfile(runOptsFor('prof'));
    if (raw) {
      profile = profileStartup(raw, {
        code: sources.code,
        map: sources.map,
        entryPath: target.entry,
      });
    }
  }

  const { report, hadBaseline } = writeNodeReport(target, summary, {
    label: opts.label,
    coverage: {
      moduleCount: cov.modules.length,
      totalBytes: cov.modules.reduce((sum, m) => sum + m.totalBytes, 0),
      readyBytes: cov.modules.reduce((sum, m) => sum + m.readyBytes, 0),
      externalScriptCount: cov.externalScriptCount,
      modules: cov.modules,
      scripts: cov.scripts,
    },
    profile: profile
      ? { totalMs: profile.totalMs, ownership: profile.ownership, rows: profile.rows.slice(0, 40) }
      : null,
  });
  printNodeSummary(report, hadBaseline);
  printNodeLeads(report, cov, profile, nodeModuleGraph(opts, target), Number(opts.top));
  if (opts.pin) pinNodeBaseline(target);
  console.log(`\nfull report: ${target.paths.metrics}`);
}

/**
 * The leads, ordered so the first question answered is whether the bundle is the cost
 * at all. A harness that always produces deferral advice will get deferral work done on
 * an app whose startup is 80% native addon loading — and the number will not move.
 */
function printNodeLeads(report, cov, profile, graph, top) {
  if (profile) {
    const own = profile.ownership;
    console.log(
      `\npre-ready CPU ${round(own.accountedMs)}ms: app modules ${round(own.appMs)}ms (${own.appShare ?? 0}%), ` +
        `runtime/native ${round(own.runtimeMs)}ms, engine ${round(own.engineMs)}ms`,
    );
    if ((own.appShare ?? 0) < 25) {
      console.log(
        '  the bundle is NOT the main cost here - most pre-ready time is outside your modules',
      );
      console.log(
        '  (native addon load, TLS/crypto init, a DB or socket handshake). Deferring imports will',
      );
      console.log('  not move startup until that work moves. Top non-app frames:');
      for (const row of profile.rows.filter((r) => !r.bucket.startsWith('(engine')).slice(0, 5)) {
        console.log(`    ${String(round(row.ms)).padStart(7)}ms  ${row.bucket}`);
      }
    } else {
      console.log('  top self-time before ready:');
      for (const row of profile.rows.slice(0, Math.min(top, 8))) {
        console.log(`    ${String(round(row.ms)).padStart(7)}ms  ${row.bucket}`);
      }
    }
  } else {
    console.log(
      `\n(no boot profile: ${path.basename(report.entry)}.map is missing - build with sourcemaps to attribute pre-ready CPU to modules)`,
    );
  }

  const cold = coldModules(cov.modules);
  const totalBytes = cov.modules.reduce((sum, m) => sum + m.totalBytes, 0);
  const readyBytes = cov.modules.reduce((sum, m) => sum + m.readyBytes, 0);
  console.log(
    `\nevaluated to become ready: ${kb(readyBytes)} of ${kb(totalBytes)} attributed ` +
      `(${cov.modules.length} modules; ${cov.externalScriptCount} non-local script(s) not attributed)`,
  );
  if (cold.length) {
    console.log(
      `\ncold at ready - shipped into the startup path, never executed to get there (${cold.length}):`,
    );
    for (const mod of cold.slice(0, top)) {
      const priced = graph ? priceDeferral(graph, mod.source) : null;
      console.log(
        `  ${kb(mod.totalBytes).padStart(10)}  ${mod.source}${priced ? `  -> deferring frees ${kb(priced.removedBytes)} / ${priced.removedCount} module(s)` : ''}`,
      );
    }
    console.log(
      `  next: ${CLI} what-if <module> prices the exact deferral; move the import into the handler that needs it.`,
    );
  } else {
    console.log(
      '\ncold at ready: none above the floor - the startup path is not carrying dead weight.',
    );
  }

  if (!graph) {
    console.log(
      '\n(no module-graph.json: a rolldown devtools report build would price every deferral statically -' +
        ' set devtools = { mode: "report" } and rebuild, then `graph` / `what-if` / `cut` work on this target)',
    );
  }
}

/** Price a coverage-derived module against the build graph (see `graphIdCandidates`). */
function priceDeferral(graph, source) {
  for (const form of graphIdCandidates(source)) {
    const hit = resolveModule(graph, form);
    if (!hit || hit.ambiguous) continue;
    const result = whatIf(graph, hit.index);
    if (result.removedCount) return result;
  }
  return null;
}

const round = (v) => (v == null ? 0 : Math.round(v));

// --- Cloudflare Worker startup mode ----------------------------------------------
// The question is the server one ("what does startup pay for, and what can leave it?")
// asked of a V8 isolate coming up under workerd. The target is a Vite-built Worker: the
// output directory that holds the plugin's `wrangler.json` next to `index.js`. perf-kit
// runs the app's own workerd through its miniflare; see lib/worker/launch.mjs for why
// startup is moved inside a request. Requirements and rules: AGENTS.md.

const WORKER_TARGET_FILE = path.join(STATE_DIR, 'worker-target.json');
const WORKER_TARGET_OPTS = {
  app: { type: 'string' },
  dist: { type: 'string' },
  worker: { type: 'string' },
  main: { type: 'string' },
  request: { type: 'string', multiple: true },
  tools: { type: 'string' },
  report: { type: 'string' },
};

function workerTargetPaths(outDir) {
  const dir = path.join(STATE_DIR, 'worker', targetSlug(outDir));
  return {
    dir,
    metrics: path.join(dir, 'startup-metrics.json'),
    state: path.join(dir, '.state.json'),
    baseline: path.join(dir, 'baseline.json'),
    history: path.join(dir, 'history.jsonl'),
    profile: path.join(dir, 'startup.cpuprofile'),
    coverage: path.join(dir, 'startup-coverage.json'),
    attribProfile: path.join(dir, 'attrib.cpuprofile'),
  };
}

/** Where the Worker's build report lands: report mode writes one directory per output dir. */
function workerReportDirs(appDir, outDir, report = null) {
  const dirs = [];
  if (report) dirs.push(path.resolve(report));
  dirs.push(reportDirFor(appDir, outDir));
  return dirs.map((d) => path.join(d, 'module-graph.json'));
}

/** The remembered Worker target's report candidates, when the query is about it. */
function workerReportCandidates(opts) {
  const sticky = readJson(WORKER_TARGET_FILE);
  if (!sticky) return null;
  if (opts.app && path.resolve(opts.app) !== sticky.app) return null;
  if (opts.dist) return null;
  const browser = readJson(TARGET_FILE);
  if (!opts.app && browser?.dist && fs.existsSync(browser.dist)) return null;
  return workerReportDirs(
    sticky.app,
    path.resolve(sticky.app, sticky.dist ?? 'dist', sticky.worker),
  );
}

/**
 * Resolve (and remember) the Worker target: the app directory, the output directory
 * that holds `wrangler.json`, the pinned request set and an optional tools directory.
 * Like the server target, what is pinned is what makes two scans comparable.
 */
function resolveWorkerTarget(opts) {
  const sticky = readJson(WORKER_TARGET_FILE);
  const appArg = opts.app ?? sticky?.app;
  if (!appArg) {
    throw new Error(
      'no Worker target yet - name the app directory (the one with vite.config and wrangler.jsonc):\n' +
        `  ${CLI} worker-scan --app <appDir>\n` +
        `Build it first with the report on: \`${CLI} worker-config --app <appDir>\` prints the build command.`,
    );
  }
  const appDir = path.resolve(appArg);
  const distRel = opts.dist ?? sticky?.dist ?? 'dist';
  const distDir = path.resolve(appDir, distRel);
  let workerName = opts.worker ?? sticky?.worker;
  if (!workerName || !fs.existsSync(path.join(distDir, workerName, 'wrangler.json'))) {
    const candidates = fs.existsSync(distDir)
      ? fs.readdirSync(distDir).filter((d) => fs.existsSync(path.join(distDir, d, 'wrangler.json')))
      : [];
    if (candidates.length !== 1) {
      throw new Error(
        `expected one ${distRel}/<worker>/wrangler.json under ${appDir}, found ${candidates.length ? candidates.join(', ') : 'none'}.\n` +
          `Build the app with the Cloudflare Vite plugin first (\`${CLI} worker-config --app <appDir>\` prints the command),\n` +
          'or pass --worker <name> / --dist <dir>. Never change the build layout to fit this tool.',
      );
    }
    workerName = candidates[0];
  }
  const outDir = path.join(distDir, workerName);
  const requests = opts.request?.length ? opts.request : (sticky?.requests ?? ['/']);
  const toolsDir = opts.tools ? path.resolve(opts.tools) : (sticky?.tools ?? null);
  const target = {
    appDir,
    distRel,
    distDir,
    workerName,
    outDir,
    configPath: path.join(outDir, 'wrangler.json'),
    requests,
    toolsDir,
    reportDirs: workerReportDirs(appDir, outDir, opts.report),
    paths: workerTargetPaths(outDir),
  };
  const changed =
    sticky?.app !== appDir ||
    sticky?.dist !== distRel ||
    sticky?.worker !== workerName ||
    JSON.stringify(sticky?.requests) !== JSON.stringify(requests) ||
    (sticky?.tools ?? null) !== toolsDir;
  if (changed) {
    writeJson(WORKER_TARGET_FILE, {
      app: appDir,
      dist: distRel,
      worker: workerName,
      requests,
      tools: toolsDir,
      setAtMs: Date.now(),
    });
    console.log(
      `target: ${path.relative(process.cwd(), outDir) || outDir} (requests: ${requests.join(' ')}${toolsDir ? `; coverage tools: ${toolsDir}` : ''}) - remembered`,
    );
  }
  return target;
}

/** Load the runtime and the Worker's options; `outDir` may be an attribution build's. */
async function loadWorkerContext(target, { outDir = target.outDir, main = null } = {}) {
  // Miniflare caches the `cf` object under the cwd (.wrangler/cache) unless told where;
  // perf-kit's state dir keeps that out of the package and out of the user's project.
  process.env.MINIFLARE_CACHE_DIR ??= path.join(STATE_DIR, 'worker', 'cache');
  const tools = await resolveWorkerTools(target.appDir).load();
  const configPath = path.join(outDir, 'wrangler.json');
  if (!fs.existsSync(configPath)) throw new Error(`no wrangler.json in ${outDir}`);
  const { workerOptions, main: configMain, fallback } = loadWorkerOptions(tools, configPath);
  const mainName = (main ?? path.relative(outDir, path.resolve(outDir, configMain))).replaceAll(
    '\\',
    '/',
  );
  const { modules, skipped } = classifyModules(outDir, workerOptions.modulesRules);
  if (!modules.some((m) => m.rel === mainName)) {
    throw new Error(
      `main module ${mainName} is not among the output modules of ${outDir} (pass --main <file>)`,
    );
  }
  const borrowed = target.toolsDir ? await resolveWorkerTools(target.toolsDir).load() : null;
  if (fallback) {
    console.log(
      `note: wrangler could not express every binding (${fallback}); running with date, flags and modules only`,
    );
  }
  return {
    ...target,
    outDir,
    tools,
    borrowed,
    workerOptions,
    mainName,
    modules,
    skipped,
    fallback,
    stderr: createStderrCollector(),
  };
}

function printWorkerContext(ctx) {
  const v = ctx.tools.versions;
  const flags = ctx.workerOptions.compatibilityFlags ?? [];
  console.log(
    `${path.relative(ctx.appDir, ctx.outDir)}/${ctx.mainName} · compat ${ctx.workerOptions.compatibilityDate ?? '?'} [${flags.join(', ')}] · ` +
      `wrangler ${v.wrangler ?? '?'} / workerd ${v.workerd ?? '?'}${ctx.borrowed ? ` (coverage tools: workerd ${ctx.borrowed.versions.workerd})` : ''} · ` +
      `${ctx.modules.length} module(s)${ctx.skipped.length ? `, skipped ${ctx.skipped.join(', ')}` : ''}`,
  );
}

async function runWorkerTiming(ctx, { runs, warmup, pinned = null }) {
  process.stderr.write('floor (the wrapper importing an empty module)...\n');
  const floor = await gatherSeries(ctx, { target: EMPTY_NAME, runs, warmup });
  if (!pinned) {
    process.stderr.write('main module...\n');
    const main = await gatherSeries(ctx, {
      target: ctx.mainName,
      runs,
      warmup,
      onSample: (i, run) => process.stderr.write(`run ${i + 1}/${runs}: import ${ms(run.ms)}\n`),
    });
    return summarizeWorkerStartup({ floor, main });
  }
  // The pinned build runs in the same scan, alternating with the current one (ABBA), so a
  // change in machine load moves both series alike instead of the verdict.
  process.stderr.write('main module, alternating with the pinned build...\n');
  for (let i = 0; i < warmup; i++) {
    await timedImport(ctx, { target: ctx.mainName });
    await timedImport(pinned, { target: pinned.mainName });
  }
  const main = [];
  const base = [];
  for (let i = 0; i < runs; i++) {
    const order = i % 2 === 0 ? [ctx, pinned] : [pinned, ctx];
    for (const c of order) {
      const run = await timedImport(c, { target: c.mainName });
      (c === ctx ? main : base).push(run);
    }
    process.stderr.write(
      `run ${i + 1}/${runs}: import ${ms(main[i].ms)} (pinned build ${ms(base[i].ms)})\n`,
    );
  }
  const summary = summarizeWorkerStartup({ floor, main });
  const pinnedSummary = summarizeWorkerStartup({ floor, main: base });
  const pairs = main.map((r, i) => Math.round((r.ms - base[i].ms) * 10) / 10);
  summary.pinnedBuild = {
    appStartupMs: pinnedSummary.metrics['worker.app_startup_ms'],
    deltaMs: median(pairs),
    pairs,
  };
  return summary;
}

/** The pinned build's copy, so later scans can time it next to the current build. */
function pinnedBuildDir(ctx) {
  return path.join(ctx.paths.dir, 'pinned-build');
}

function snapshotPinnedBuild(ctx) {
  const dest = pinnedBuildDir(ctx);
  fs.rmSync(dest, { recursive: true, force: true });
  fs.cpSync(ctx.outDir, dest, { recursive: true, filter: (src) => !src.endsWith('.map') });
  // wrangler resolves the assets directory next to the config: keep it on the original.
  const configFile = path.join(dest, 'wrangler.json');
  const config = JSON.parse(fs.readFileSync(configFile, 'utf8'));
  if (config.assets?.directory) {
    config.assets.directory = path.resolve(ctx.outDir, config.assets.directory);
    fs.writeFileSync(configFile, JSON.stringify(config));
  }
}

/** The context of the pinned build's copy, or null when there is none or it does not load. */
async function loadPinnedContext(ctx, notes) {
  const dir = pinnedBuildDir(ctx);
  if (!fs.existsSync(path.join(dir, 'wrangler.json'))) return null;
  try {
    const pinned = await loadWorkerContext(ctx, { outDir: dir, main: ctx.mainName });
    return { ...pinned, stderr: ctx.stderr };
  } catch (error) {
    notes.push(
      `the pinned build did not load, compared by number only: ${String(error.message).split('\n')[0]}`,
    );
    return null;
  }
}

function writeWorkerReport(
  ctx,
  summary,
  { label, requests = null, profile = null, coverage = null, notes = [] },
) {
  const prev = readJson(ctx.paths.state);
  const baseline = readJson(ctx.paths.baseline);
  const metrics = { ...summary.metrics };
  for (const r of requests ?? []) if (r.status < 500) metrics[`worker.request_ms:${r.path}`] = r.ms;
  const report = {
    schemaVersion: 1,
    generatedAtMs: Date.now(),
    label: label || null,
    worker: {
      app: ctx.appDir,
      outDir: ctx.outDir,
      main: ctx.mainName,
      compatibilityDate: ctx.workerOptions.compatibilityDate ?? null,
      compatibilityFlags: ctx.workerOptions.compatibilityFlags ?? [],
      versions: ctx.tools.versions,
      coverageTools: ctx.borrowed?.versions ?? null,
    },
    runs: summary.runs,
    metrics,
    guard: summary.guard,
    requests,
    profile,
    coverage,
    notes,
    delta: prev ? deltaSection(prev.metrics, metrics) : null,
    baselineDelta: baseline ? deltaSection(baseline.metrics, metrics) : null,
    pinnedBuild: summary.pinnedBuild ?? null,
    samples: summary.samples,
    runtimeStderr: [...ctx.stderr.lines.entries()],
  };
  writeJson(ctx.paths.metrics, report);
  writeJson(ctx.paths.state, {
    schemaVersion: 1,
    tsMs: report.generatedAtMs,
    label: report.label,
    runs: report.runs,
    metrics,
  });
  fs.appendFileSync(
    ctx.paths.history,
    `${JSON.stringify({ tsMs: report.generatedAtMs, label: report.label, metrics, guard: report.guard })}\n`,
  );
  return { report, hadBaseline: Boolean(baseline) };
}

function pinWorkerBaseline(ctx) {
  const state = readJson(ctx.paths.state);
  if (!state) throw new Error('nothing to pin - run worker-scan first');
  if (state.runs < 3) {
    throw new Error(
      `refusing to pin a ${state.runs}-run measurement - a thin median poisons every later delta. Re-run worker-scan with the default sample count.`,
    );
  }
  writeJson(ctx.paths.baseline, state);
  snapshotPinnedBuild(ctx);
  console.log(
    `baseline pinned (app startup ${ms(state.metrics['worker.app_startup_ms'])}); later scans time this build next to theirs`,
  );
}

function noiseCall(delta, prev, floorMs, floorPct) {
  const floor = Math.max(floorMs, (floorPct / 100) * prev);
  if (Math.abs(delta) < floor)
    return `within noise (floor ${Math.round(floor)}ms) - no effect proven`;
  return delta < 0 ? 'IMPROVEMENT beyond noise' : 'REGRESSION beyond noise';
}

function printWorkerSummary(report, hadBaseline) {
  const m = report.metrics;
  console.log(
    `\n${report.label ? `[${report.label}] ` : ''}${report.runs} cold runs (fresh workerd each)`,
  );
  console.log(
    `startup ${ms(m['worker.startup_ms'])} (p75 ${ms(m['worker.startup_p75_ms'])}) | floor ${ms(m['worker.boot_floor_ms'])} | ` +
      `app ${ms(m['worker.app_startup_ms'])} | instance boot ${ms(m['worker.instance_boot_ms'])} (not counted)`,
  );
  const spread = report.guard.spreadPct;
  console.log(
    `guard: ${report.guard.allRunsCompleted ? 'PASS' : 'FAIL (a run never imported the main module)'}` +
      `${spread > 25 ? ` - WARNING spread ${spread}% across runs: something outside the bundle moves this number, deltas are not trustworthy` : ''}`,
  );
  const base = report.baselineDelta?.['worker.app_startup_ms'];
  const paired = report.pinnedBuild;
  if (paired) {
    const sign = (v) => `${v >= 0 ? '+' : ''}${Math.round(v * 10) / 10}`;
    console.log(
      `vs pinned build (timed in this scan, runs alternated): app startup ${sign(paired.deltaMs)}ms, ` +
        `median of ${paired.pairs.length} pairs [${paired.pairs.map(sign).join(' ')}] - ` +
        noiseCall(
          paired.deltaMs,
          paired.appStartupMs,
          WORKER_NOISE_FLOOR_MS,
          WORKER_NOISE_FLOOR_PCT,
        ),
    );
    console.log(
      '  every change since the pin; re-pin with --pin after a kept change so this is the last change alone',
    );
  } else if (base) {
    console.log(
      `vs pinned baseline: app startup ${base.delta >= 0 ? '+' : ''}${Math.round(base.delta)}ms (${base.pct}%) - ` +
        noiseCall(base.delta, base.prev, WORKER_NOISE_FLOOR_MS, WORKER_NOISE_FLOOR_PCT) +
        ' (a number from an earlier scan: machine load moves it; re-pin to time both builds side by side)',
    );
  } else if (!hadBaseline) {
    console.log('no pinned baseline yet - pass --pin to make this the fixed reference');
  }
  // The pinned line adds up every change since the pin. The last change alone is the
  // delta against the previous scan, so a kept-but-not-pinned change does not hide it.
  // Next to a pinned build timed in the same scan, a number from another scan only adds drift.
  const prevScan = report.delta?.['worker.app_startup_ms'];
  if (!paired && base && prevScan && prevScan.prev !== base.prev) {
    console.log(
      `vs previous scan: app startup ${prevScan.delta >= 0 ? '+' : ''}${Math.round(prevScan.delta)}ms (${prevScan.pct}%) - ` +
        noiseCall(prevScan.delta, prevScan.prev, WORKER_NOISE_FLOOR_MS, WORKER_NOISE_FLOOR_PCT) +
        ' (the last change alone)',
    );
  }
  if (report.requests?.length) {
    console.log(
      '\nfirst requests on the real entry (startup already done; a deferred module loads on its first hit):',
    );
    const startupDelta = paired ? paired.deltaMs : base?.delta;
    const gain = startupDelta < 0 ? -startupDelta : 0;
    const movedRoutes = [];
    for (const r of report.requests) {
      const d = report.baselineDelta?.[`worker.request_ms:${r.path}`];
      // A 4xx or 5xx answer never reached the app's handler: its time says nothing about a moved load.
      const answered = r.status < 400;
      const moved =
        answered && d && Math.abs(d.delta) >= WORKER_NOISE_FLOOR_MS
          ? `  vs baseline ${d.delta >= 0 ? '+' : ''}${Math.round(d.delta)}ms${d.delta > 0 ? ' - cost MOVED here from startup' : ''}`
          : '';
      if (answered && d && gain && d.delta >= gain / 2)
        movedRoutes.push({ path: r.path, delta: d.delta });
      console.log(
        `  ${(r.path.length > 40 ? `${r.path.slice(0, 39)}…` : r.path).padEnd(28)} ${String(r.status).padStart(3)}  ${ms(r.ms).padStart(7)}  ${r.bytes} bytes${r.messages != null ? ` in ${r.messages} message(s)` : ''}${r.error ? `  :: ${r.error}` : ''}${moved}`,
      );
    }
    if (report.requests.every((r) => r.status >= 400)) {
      console.log(
        '  every pinned route answered 4xx/5xx: the handler never ran, so a moved load cannot show here - pin routes the app answers (--request /path)',
      );
    }
    if (report.requests.some((r) => r.status >= 500)) {
      console.log(
        '  a 5xx here usually means the app needs its variables (a .dev.vars next to the config) - startup is unaffected, first-request numbers are not valid',
      );
    }
    for (const r of movedRoutes) {
      console.log(
        `  ${r.path}: +${Math.round(r.delta)}ms moved into this route against ${Math.round(gain)}ms gained at startup - an isolate whose first request is ${r.path} gains little or nothing`,
      );
    }
  }
}

/**
 * Calls made at module scope. The self-time view charges a warm-up such as
 * `preloadSchemas()` to the files it runs in (zod); this view names the call.
 */
function printTopLevelCalls(calls, { top, withSource }) {
  // One row per call site: the same library function called from several modules adds up.
  const merged = new Map();
  for (const c of calls ?? []) {
    const key = `${c.fn}\0${c.at}`;
    const row = merged.get(key);
    if (row) row.ms = Math.round((row.ms + c.ms) * 10) / 10;
    else merged.set(key, { ...c });
  }
  const rows = [...merged.values()]
    .sort((a, b) => b.ms - a.ms)
    .filter((c) => c.ms >= 0.5)
    .slice(0, top);
  if (!rows.length) return;
  console.log('  calls made at module scope (the call right below a module top level):');
  for (const c of rows) {
    const where =
      withSource && c.source ? `${shortModule(c.source)}  (${c.at})` : shortModule(c.at);
    console.log(`    ${String(c.ms).padStart(7)}ms  ${c.fn}()  ${where}`);
  }
  const lead = rows[0];
  if (lead.ms >= 2 && /node_modules\//.test(lead.source ?? lead.at)) {
    console.log(
      `  \`${lead.fn}\` is library code that runs at module scope: read it before you call the cost upstream (AGENTS.md, "Library top-level work")`,
    );
  }
}

/**
 * The leads, in the order a reader should take them: is the Worker's own code the cost,
 * which modules run at startup for how long, and what is parsed but not executed.
 */
function printWorkerLeads(report, cov, prof, graph, top) {
  if (prof?.rows) {
    const own = prof.ownership;
    const sm = prof.summary;
    console.log(
      `\nstartup CPU (profile): active ${ms(sm.activeMs)} = app modules ${ms(own.appMs)} (${own.appShare ?? 0}%) + runtime ${ms(own.runtimeMs)} + engine ${ms(own.engineMs)}` +
        ` + parse/compile "(program)" ${ms(sm.programMs)}; GC ${ms(sm.gcMs)}; ${sm.samples} samples`,
    );
    console.log('  top self-time at startup:');
    for (const row of prof.rows.slice(0, Math.min(top, 10))) {
      console.log(`    ${String(round(row.ms)).padStart(7)}ms  ${row.bucket}`);
    }
    printTopLevelCalls(prof.calls, { top: Math.min(top, 8), withSource: true });
    const unmapped = prof.rows.find((r) => r.bucket === '(unmapped)');
    if (unmapped && unmapped.ms >= 2) {
      console.log(
        `  ${ms(unmapped.ms)} sits in chunk top-level code V8 gave no line ticks for - \`${CLI} worker-attrib\` on an attribution build charges it to the evaluating module`,
      );
    }
  } else if (prof?.missingSourcemap) {
    console.log(
      `\n(no startup CPU by module: ${report.worker.main}.map is missing - build the worker environment with sourcemaps; \`${CLI} worker-config\` sets that)`,
    );
  }

  if (cov?.totals) {
    const t = cov.totals;
    console.log(
      `\nevaluated at startup: ${kb(t.readyBytes)} of ${kb(t.totalBytes)} in the static closure (${t.moduleCount} modules, ${Math.round((t.readyBytes / Math.max(1, t.totalBytes)) * 100)}% executed)` +
        `${cov.runtimeScripts.length ? `; ${cov.runtimeScripts.length} runtime module(s) loaded (${cov.runtimeScripts.slice(0, 5).join(', ')}${cov.runtimeScripts.length > 5 ? ', ...' : ''})` : ''}`,
    );
    if (cov.cold.length) {
      console.log(
        `\ncold at startup - in the closure, never executed to start (${cov.cold.length}):`,
      );
      for (const mod of cov.cold.slice(0, top)) {
        const priced = graph ? priceDeferral(graph, mod.source) : null;
        console.log(
          `  ${kb(mod.totalBytes).padStart(10)}  ${mod.source}${priced ? `  -> deferring frees ${kb(priced.removedBytes)} / ${priced.removedCount} module(s)` : ''}`,
        );
      }
    }
    const idle = cov.idle
      .filter((m) => m.idleBytes >= 16 * 1024 && m.readyRatio > 0.02)
      .slice(0, top);
    if (idle.length) {
      console.log(
        '\nparsed but mostly not executed at startup (libraries run a slice of themselves at import; the rest is load cost):',
      );
      for (const mod of idle) {
        const priced = graph ? priceDeferral(graph, mod.source) : null;
        console.log(
          `  ${kb(mod.idleBytes).padStart(10)} idle of ${kb(mod.totalBytes).padStart(9)}  ${String(Math.round(mod.readyRatio * 100)).padStart(3)}% executed  ${mod.source}${priced ? `  -> deferring frees ${kb(priced.removedBytes)}` : ''}`,
        );
      }
    }
    console.log('\nby package (bytes in the startup closure, share executed):');
    for (const p of cov.packages.slice(0, Math.min(top, 12))) {
      console.log(
        `  ${kb(p.totalBytes).padStart(10)}  ${String(Math.round((p.readyBytes / Math.max(1, p.totalBytes)) * 100)).padStart(3)}%  ${String(p.modules).padStart(4)} module(s)  ${p.pkg}`,
      );
    }
  }

  if (!graph) {
    const cause = reportSupportNote(ctx.appDir, CLI);
    console.log(
      cause
        ? `\n(no module-graph.json for this worker)\n${cause}`
        : `\n(no module-graph.json for this worker: \`${CLI} worker-config --app <appDir>\` writes a build config that collects it; then graph / what-if / cut / why price every candidate)`,
    );
  } else {
    console.log(
      `\nnext: \`${CLI} why <module>\` names the importers and the chain from the entry; \`${CLI} cut <module> --root <class modules>\` the fewest imports to make dynamic.` +
        ' Durable Object and Workflow classes are loaded by name: pass the modules that define them as --root, or every cut points at the entry.',
    );
    const classes = workerClassNames(report.worker.outDir);
    if (classes.length) console.log(`  class exports in wrangler.json: ${classes.join(', ')}`);
  }
}

/** Durable Object and Workflow class names the config binds; their modules are roots. */
function workerClassNames(outDir) {
  const config = readJson(path.join(outDir, 'wrangler.json'));
  if (!config) return [];
  const names = new Set();
  for (const b of config.durable_objects?.bindings ?? []) if (b.class_name) names.add(b.class_name);
  for (const w of config.workflows ?? []) if (w.class_name) names.add(w.class_name);
  return [...names];
}

function workerModuleGraph(ctx) {
  return loadModuleGraph(ctx.reportDirs);
}

function printRuntimeStderr(ctx) {
  if (!ctx.stderr.lines.size) return;
  console.log('\nworkerd stderr (deduplicated):');
  for (const [line, n] of ctx.stderr.lines) console.log(`  ${n}x ${line}`);
}

async function cmdWorkerMeasure(argv) {
  const opts = parse(argv, {
    ...WORKER_TARGET_OPTS,
    runs: { type: 'string', default: '5' },
    warmup: { type: 'string', default: '1' },
    label: { type: 'string', default: '' },
    pin: { type: 'boolean', default: false },
  });
  const ctx = await loadWorkerContext(resolveWorkerTarget(opts), { main: opts.main });
  printWorkerContext(ctx);
  const summary = await runWorkerTiming(ctx, {
    runs: Number(opts.runs),
    warmup: Number(opts.warmup),
  });
  const requests = await firstRequests(ctx, ctx.requests);
  const { report, hadBaseline } = writeWorkerReport(ctx, summary, { label: opts.label, requests });
  printWorkerSummary(report, hadBaseline);
  if (opts.pin) pinWorkerBaseline(ctx);
  printRuntimeStderr(ctx);
  console.log(`\nfull report: ${ctx.paths.metrics}`);
}

async function cmdWorkerScan(argv) {
  const opts = parse(argv, {
    ...WORKER_TARGET_OPTS,
    runs: { type: 'string', default: '5' },
    warmup: { type: 'string', default: '1' },
    label: { type: 'string', default: '' },
    pin: { type: 'boolean', default: false },
    top: { type: 'string', default: '10' },
    interval: { type: 'string', default: '100' },
    'no-coverage': { type: 'boolean', default: false },
    'no-profile': { type: 'boolean', default: false },
  });
  const ctx = await loadWorkerContext(resolveWorkerTarget(opts), { main: opts.main });
  printWorkerContext(ctx);
  const notes = [];
  const pinned = await loadPinnedContext(ctx, notes);
  const summary = await runWorkerTiming(ctx, {
    runs: Number(opts.runs),
    warmup: Number(opts.warmup),
    pinned,
  });
  process.stderr.write('first requests on the real entry...\n');
  const requests = await firstRequests(ctx, ctx.requests);

  let prof = null;
  if (!opts['no-profile']) {
    process.stderr.write('profile run (where the startup CPU went)...\n');
    const { profile, importRan, importText } = await cpuProfileRun(ctx, {
      intervalUs: Number(opts.interval),
    });
    if (!importRan)
      notes.push(`profile run: the import did not complete: ${importText.slice(0, 200)}`);
    fs.mkdirSync(ctx.paths.dir, { recursive: true });
    fs.writeFileSync(ctx.paths.profile, JSON.stringify(profile));
    prof = attributeProfile(profile, { outDir: ctx.outDir, mainName: ctx.mainName });
    prof.calls = topLevelCalls(profile, outputLocator(ctx.outDir));
  }

  let cov = null;
  if (!opts['no-coverage']) {
    process.stderr.write(
      'coverage run (what executed at startup; in-process inspector session)...\n',
    );
    const run = await workerCoverageRun(ctx, { tools: ctx.borrowed });
    if (run.unsupported) {
      notes.push(`precise coverage unavailable: ${run.unsupported}`);
    } else {
      if (run.importError)
        notes.push(`coverage run: the import threw: ${run.importError.slice(0, 200)}`);
      fs.mkdirSync(ctx.paths.dir, { recursive: true });
      fs.writeFileSync(ctx.paths.coverage, JSON.stringify(run.result));
      cov = attributeCoverage(run.result, { outDir: ctx.outDir });
      cov.source = `precise, workerd ${run.workerd}${run.borrowed ? ' (borrowed through --tools)' : ''}; flags added for that run: ${run.addedFlags.join(', ') || 'none'}`;
    }
  }

  const graph = workerModuleGraph(ctx);
  const { report, hadBaseline } = writeWorkerReport(ctx, summary, {
    label: opts.label,
    requests,
    profile: prof
      ? {
          summary: prof.summary,
          ownership: prof.ownership,
          totalMs: prof.totalMs,
          rows: prof.rows?.slice(0, 40) ?? null,
        }
      : null,
    coverage: cov
      ? {
          source: cov.source,
          totals: cov.totals,
          runtimeScripts: cov.runtimeScripts,
          modules: cov.modules.slice(0, 60),
          cold: cov.cold.slice(0, 20),
          idle: cov.idle.slice(0, 20),
          packages: cov.packages.slice(0, 30),
        }
      : null,
    notes,
  });
  printWorkerSummary(report, hadBaseline);
  printWorkerLeads(report, cov, prof, graph, Number(opts.top));
  if (cov?.source) console.log(`\ncoverage: ${cov.source}`);
  for (const note of notes) console.log(`note: ${note}`);
  if (opts.pin) pinWorkerBaseline(ctx);
  printRuntimeStderr(ctx);
  console.log(`\nfull report: ${ctx.paths.metrics}`);
}

/**
 * CPU by EVALUATING module, from the attribution build (every module its own file):
 * `ROLLDOWN_PERF_KIT_ATTRIB=1 npx vite build --config perf-kit.vite.config.ts`.
 */
async function cmdWorkerAttrib(argv) {
  const opts = parse(argv, {
    ...WORKER_TARGET_OPTS,
    'attrib-dir': { type: 'string', default: ATTRIB_DIR },
    interval: { type: 'string', default: '100' },
    top: { type: 'string', default: '18' },
  });
  const target = resolveWorkerTarget(opts);
  const outDir = attribOutDir(path.join(target.appDir, opts['attrib-dir']), target.workerName);
  if (!fs.existsSync(path.join(outDir, 'wrangler.json'))) {
    throw new Error(
      `no attribution build at ${outDir}.\n` +
        `Build one: ROLLDOWN_PERF_KIT_ATTRIB=1 npx vite build --config ${SHIM_FILE}   (in ${target.appDir}; \`${CLI} worker-config\` writes that config)`,
    );
  }
  const ctx = await loadWorkerContext(target, { outDir, main: opts.main });
  printWorkerContext(ctx);
  process.stderr.write('profile run on the attribution build...\n');
  const { profile, importRan, importText } = await cpuProfileRun(ctx, {
    intervalUs: Number(opts.interval),
  });
  if (!importRan) {
    const first = importText
      .replace(/^startup threw: /, '')
      .split('\n')[0]
      .slice(0, 200);
    console.log(`\nthe attribution build did not finish its import: ${first}`);
    console.log('  the rows below cover only the modules that evaluated before that point.');
    if (/\brequire_\w+ is not a function/.test(importText)) {
      console.log(
        '  (a CommonJS module in an import cycle: with preserveModules every file evaluates\n' +
          '  `export default require_x()` eagerly, and a cycle reaches a wrapper before its own file ran.\n' +
          `  That is the bundler's output, not the app; the bundled profile of \`${CLI} worker-scan\` is the fallback.)`,
      );
    }
  }
  fs.mkdirSync(ctx.paths.dir, { recursive: true });
  fs.writeFileSync(ctx.paths.attribProfile, JSON.stringify(profile));
  const graph = loadModuleGraph(workerReportDirs(target.appDir, outDir, opts.report));
  const { rows, pairs } = evaluatingModules(profile, { outDir, graph, top: Number(opts.top) });
  const calls = topLevelCalls(profile, outputLocator(outDir));
  console.log(
    `\nstartup CPU by evaluating module${importRan ? '' : ' (partial)'} (its own top level plus the library calls it makes):`,
  );
  for (const r of rows) {
    console.log(`  ${String(r.ms).padStart(7)}ms  ${r.module}`);
    if (r.chain?.length) console.log(`             via ${r.chain.join(' -> ')}`);
    const own = calls.filter((c) => c.module === r.module && c.ms >= 0.5).slice(0, 2);
    for (const c of own)
      console.log(`             calls ${c.fn}() ${c.ms}ms at ${shortModule(c.at)}`);
  }
  console.log();
  printTopLevelCalls(calls, { top: 6, withSource: false });
  console.log('\nevaluating module -> library it spends the time in:');
  for (const p of pairs.slice(0, 12)) console.log(`  ${String(p.ms).padStart(7)}ms  ${p.pair}`);
  if (!graph) {
    console.log(
      `\n(no report for the attribution build - the chains need ${path.relative(target.appDir, reportDirFor(target.appDir, outDir))})`,
    );
  }
  printRuntimeStderr(ctx);
  console.log(`\nprofile: ${ctx.paths.attribProfile}`);
}

/** How to name an app directory in a hint: relative when it is below cwd, absolute otherwise. */
function appArgFor(appDir) {
  const rel = path.relative(process.cwd(), appDir);
  return !rel ? '.' : rel.startsWith('..') ? appDir : rel;
}

/** Write the Vite config shim and print the three build commands. */
async function cmdWorkerConfig(argv) {
  const opts = parse(argv, { app: { type: 'string' }, force: { type: 'boolean', default: false } });
  const sticky = readJson(WORKER_TARGET_FILE);
  const appDir = path.resolve(opts.app ?? sticky?.app ?? '.');
  const { file, baseConfigFile, existed } = writeViteConfigShim(appDir, { force: opts.force });
  console.log(
    `${existed && !opts.force ? 'kept' : 'wrote'} ${file} (wraps ${baseConfigFile}; --force rewrites)`,
  );
  const cause = reportSupportNote(appDir, CLI);
  if (cause) console.log(`\nbefore you build:\n${cause}`);
  console.log(`\nbuild commands (run in ${appDir}):`);
  console.log(
    `  npx vite build --config ${SHIM_FILE}                            full build; report per environment, sourcemaps on the worker`,
  );
  console.log(
    `  ROLLDOWN_PERF_KIT_WORKER_ONLY=1 npx vite build --config ${SHIM_FILE}  rebuild the worker environment only (the loop)`,
  );
  console.log(
    `  ROLLDOWN_PERF_KIT_ATTRIB=1 npx vite build --config ${SHIM_FILE}       attribution build into ${ATTRIB_DIR}/ for \`${CLI} worker-attrib\``,
  );
  console.log(
    `\nthen: ${CLI} worker-scan --app ${appArgFor(appDir)} [--request /path ...] [--tools <dir with wrangler >= ${COVERAGE_MIN_WRANGLER}>]`,
  );
}

/** Copy this checkout's built rolldown over the copy the app's vite resolves. */
async function cmdRolldownInstall(argv) {
  const opts = parse(argv, { app: { type: 'string' }, from: { type: 'string' } });
  const sticky = readJson(WORKER_TARGET_FILE);
  const appDir = path.resolve(opts.app ?? sticky?.app ?? '.');
  const { from, dir, before, after } = installCheckoutRolldown(
    appDir,
    opts.from ? { checkout: path.resolve(opts.from) } : {},
  );
  console.log(
    `copied ${from.dir} (rolldown ${from.version}, ${from.binding}) over ${dir} (was rolldown ${before})`,
  );
  const loaded = spawnSync(
    process.execPath,
    ['-e', "import('rolldown').then((m) => console.log(m.VERSION))"],
    { cwd: appDir, encoding: 'utf8' },
  );
  const version = loaded.status === 0 ? loaded.stdout.trim() : null;
  if (!version) {
    throw new Error(
      `the copy does not load from ${appDir}: ${(loaded.stderr || '').split('\n').find(Boolean) ?? 'no output'}`,
    );
  }
  console.log(
    after.reportMode
      ? `rolldown ${version} with the report mode now loads from the app`
      : `rolldown ${version} loads from the app but still has no report mode - is the checkout on this branch?`,
  );
  console.log(
    '\nrebuild the app, then re-pin the baseline (the bundler changed). `<package manager> install --force` in the app restores the release.',
  );
}

/** `why <module>`: direct static importers and one chain from the entry. */
async function cmdWhy(argv) {
  const { values: opts, positionals } = parseArgs({
    args: argv,
    options: { ...TARGET_OPTS, report: { type: 'string' } },
    allowPositionals: true,
  });
  const query = positionals[0];
  if (!query) throw new Error(`usage: ${CLI} why <module> [--report <dir>]`);
  const graph = requireModuleGraph(opts);
  const hit = resolveModule(graph, query);
  if (!hit)
    throw new Error(`no module matches '${query}' (ids are project-relative, e.g. src/router.ts)`);
  if (hit.ambiguous) {
    throw new Error(
      `'${query}' is ambiguous:\n  ${hit.ambiguous.join('\n  ')}\nuse a longer suffix.`,
    );
  }
  const why = whyModule(graph, hit.index);
  console.log(`${why.module} (${kb(why.bytes)}): ${why.importers.length} static importer(s)`);
  for (const imp of why.importers.slice(0, 12))
    console.log(`  <- ${imp.module} (${kb(imp.bytes)})`);
  if (why.importers.length > 12) console.log(`  ... ${why.importers.length - 12} more`);
  const chain = why.chain.filter((id) => !/^(\\0|\0)/.test(id));
  console.log(`chain from the entry: ${[...chain, why.module].join(' -> ')}`);
  console.log(
    `\nnext: \`${CLI} cut ${why.module}\` lists the fewest imports to make dynamic; add --root <module> for a class the runtime loads by name (Durable Object, Workflow).`,
  );
}

async function cmdHelp() {
  console.log(`browser-loading perf harness - measurement + diagnosis; you drive the loop

start here (the target is remembered after the first command):
  scan --app <appDir>       3 throttled runs + coverage (+ boot profile when pre-paint
                            CPU >150ms; force with --profile) + verdict, one browser session.
                            First scan of a target auto-pins the baseline. --runs 5 for noisy pages.
  scan                      same, against the remembered target
  scan --pin                same, and re-pin the baseline afterwards (after an accepted change)
  scan --quick              1 run, no profile: a fast mid-iteration probe on slow apps.
                            Indicative only - accept/revert/pin decisions need a full scan.
  scan --full               re-scans print the compact coverage view (top 15 module rows;
                            candidates/cold/large/sibling sections always complete) - --full
                            restores the whole module table
  scan --no-early-stop      scans with a pinned baseline stop after run 1 when |dLCP| is
                            >=5x the noise threshold. This flag forces all runs to check
                            both FCP and LCP before a decision. Pins always sample fully.
  scan --net-scale <1|2|4|8>  pin the target's network-bandwidth scale explicitly.
                            First scan CALIBRATES it automatically (walks x1->x8 until
                            baseline LCP fits under 8s - only pathologically heavy apps
                            leave x1; RTT and 4x CPU never change) and PINS it: the scale
                            is part of the measurement identity for the whole session.
                            Changing it makes the pinned baseline incomparable (re-pin).
  scan --bandwidth <Mbit/s> [--rtt <ms>] [--cpu <rate>]   pin a network in real units instead
                            of a scale step: the network a deployed environment measured (a
                            record's model), e.g. --bandwidth 26 --rtt 120 --cpu 1 (1 = this
                            machine's CPU). No calibration; RTT/CPU default to 150ms / 4x.
                            Pinned and compared like --net-scale (re-pin after a change).
  verdict                   fuse the gathered signals -> OPEN/clear/UNKNOWN; the only "done" that counts

rendered output (parity) - is it still the same page? Compared ONCE, when you finish.
  The first scan/measure/parity of a target saves the build as the ORIGINAL (a file copy).
  Optimize freely; each scan only says whether the LCP element is still the original's
  (free). When done, \`scan --parity\` takes the final measurement and compares every
  pinned page with the original: first-screen + full-page pixels, page height, visible
  text, accessible names, new runtime errors, and the LCP element (a different, smaller or
  placeholder LCP element means the gain is not real). A difference starts RECOVER MODE:
  that build's LCP numbers do not count until it matches again.
  scan --parity             the final check: measurement + comparison, every pinned page
  parity [--only /a,/b] [--label x] [--exit-code]   the comparison alone (no timed runs);
                            --only re-checks some pages (a build is clear once every page
                            is), --exit-code exits 1 while a page differs (git bisect run)
  parity recover            recover mode: what to do, how to re-check, how to bisect
  parity strategy each-tweak|on-demand   in recover mode: every scan re-checks and records
                            a snapshot, or you re-check when you decide (default)
  parity history            every check with its snapshot images
  scan|parity --pages /pricing,/docs   add client-side routes (entry HTML files are found
                            automatically). Pages can be added any time, never removed.
  scan|parity --mask <selector>        mask a region the ORIGINAL renders differently on
                            every load (a live clock, a rotating banner). Masks added after
                            the original was saved are listed in every verdict.
  parity anchor             show the saved original (commit, pages, masks, LCP element)
  parity anchor --replace   HUMAN ONLY: make the current build the original
  parity approve <page> --reason "..."  HUMAN ONLY: accept one exact, intended difference

individual commands (same target rules):
  measure [--runs 5] [--label x] [--pin]    timed runs only -> LCP + "vs pinned baseline" verdict
  coverage | profile                        one signal each
  graph                                     STATIC split candidates ranked by retained size (what
                                            deferring each module removes from the initial load) -
                                            needs a rolldown devtools report build (vite >= 8:
                                            build.rolldownOptions.devtools = { mode: "report" })
  what-if <module> [<module>...] [--keep a,b] [--root a,b]  exact modules+bytes one deferral frees;
                                            sentries stay eager. TWO+ modules = one combined plan, priced
                                            with a single overlay + a vs-summed-individually line
                                            (shared internals can make the combined free exceed the sum)
  cut <module> [--keep a,b] [--root a,b]    min edge cut: the FEWEST import statements to make dynamic
                                            to detach the module (last-hop edges, the exact files to
                                            edit) + the bytes freed. Beats what-if when one upstream
                                            cut detaches a multi-path module; --keep edges are protected;
                                            --root modules stay eager with their closure (a Durable
                                            Object class loaded by name) so the cut lands below them
  what-if|cut ... --root a,b                roots also price startup code no static import reaches: a
                                            dynamic import the app runs on every start joins the
                                            entries, so "initial load" = entries + roots + closure
  why <module>                              the module's direct static importers + one chain from the
                                            entry: where it enters the initial load, and who to edit
  graph-diff [--against <file>]             what a change did to the initial load (eager set): modules
                                            that entered/left with byte deltas, vs the pinned baseline
                                            graph (or any saved sidecar). Deterministic, no browser run
  baseline                                  pin the last measurement as the fixed reference (also
                                            snapshots the module graph for graph-diff)
  target [<appDir>] [--demo]                show / set / clear the remembered target
         [--net-scale <1|2|4|8>]            ...and/or pin its throttle net-scale without probing
         [--bandwidth <Mbit/s> [--rtt <ms>] [--cpu <rate>]]   ...or pin a network in real units
  gen | build | defer <f> | undefer <f> | status | serve    demo-app helpers (README.md)

field mode (a DEPLOYED site in your own logged-in Chrome - real data, network, server):
  field --config <file.json>   alternating timed runs per variant (cookie/query switches the
                            build), the request LCP waited for and the script chain that
                            started it, then one coverage + CPU profile run per variant.
                            Writes runtime-metrics/coverage/profile.json per variant and
                            field-report.json (paired deltas, 90% intervals) OUTSIDE the repo.
                            Needs chrome://inspect/#remote-debugging on (or --cdp-port).
  --runs 6                  pairs of timed runs
  --diagnose 1|0            one coverage + CPU profile run per variant, or none
  --cut gate|lcp|fcp        coverage snapshot point; gate = when the config's "gate" request
                            is sent ("what ran before the request LCP waits for")
  --throttle lab [--net-scale n]   add the lab throttle ON TOP of the real network
  Module attribution needs "build" per variant: a local build whose chunks are
  byte-identical to the served ones (same commit, settings, bundler version).

replay (field numbers for a build that is NOT deployed):
  Standalone entry for agents (one records, another replays): node replay.mjs help
  (adds inspect, record --url/--cookie without a config, --json results, HANDOFF.txt).
  record --config <file.json> [--variant <name>] [--runs 6]
                            load one deployed variant in your Chrome (like field) and write a
                            replay archive to $TMPDIR: every response (safelisted headers, no
                            cookies), its median server time, a fitted network model, the
                            field medians. Holds REAL account data - it refuses a git work tree.
  replay --archive <dir> --dist [name=]<dir> [--dist [name=]<dir>] [--runs 6]
                            serve the archive to headless Chrome from one local HTTP/2 server
                            for every host: the local dist replaces the deployed files, API
                            responses keep their recorded server time, all bodies share the
                            recorded bandwidth. Same reports as field, paired deltas.
  replay ... --calibrate    FIRST: replay the recorded build; its medians must match the field
                            medians, or the model misses something and an A/B means nothing
  --bandwidth <Mbit/s> --rtt <ms> --static-ttfb <ms> --wire <kB> --ect <rating|off>   override the model
  --document merged|local|recorded   the dist's index.html plus the tags only the deployed
                            HTML has (default: a host's injected loaders run in the replay too),
                            the bare dist index.html, or the recorded HTML
  --storage warm|empty      warm: an unmeasured load first, in the same browser context, so the
                            build's own storage is there (default when the recorded page had storage)
  --diagnose 1 [--cut gate|lcp|fcp]   coverage + CPU profile run per dist, as in field

server startup mode (node/deno - a process coming up, not a page loading):
  node-scan --entry dist/server.js --ready port:3000    timed spawn->ready runs + what had to
                            execute to get there + where the pre-ready CPU went + leads.
                            The entry and its readiness signal are remembered; later
                            \`node-scan\` runs bare. --pin makes this the baseline.
  node-measure              timing only, no instrumented runs
  --ready <spec>            how the target signals it is up. This is part of the target -
                            two scans with different specs are not comparable:
                              port:3000            a TCP listener is accepting
                              'stdout:ready on'    a line matching this regex is printed
                              http://127.0.0.1:3000/health   an HTTP probe returns 2xx
                              exit                 the process exits 0 (CLIs, lambda entries)
  --cache cold|warm|off     V8 compile cache. cold (default) gives every run an empty cache,
                            which is what a new container pays; warm shares one primed dir.
                            Measuring warm by accident measures the cache, not the bundle.
  --args "..." --exec <bin> --cwd <dir>    argv for the entry / the runtime binary (deno,
                            an older node) / working directory
  graph | what-if | cut | graph-diff       work unchanged on a server build - pass
                            --dist <builtDir> or --report <dir>. A top-level import and a
                            render-blocking import are the same edge in the module graph.
  There is no throttle in this mode: server startup does no network I/O, so a throttle
  would invent a dimension the measurement does not have.

Cloudflare Worker startup mode (a V8 isolate coming up under the app's own workerd):
  worker-config --app <appDir>    write perf-kit.vite.config.ts next to the app's vite config:
                            the rolldown metrics report on EVERY environment (one dir each),
                            sourcemaps on the worker, a worker-only rebuild and an attribution
                            build behind env vars. Prints the build commands. Build, then:
  worker-scan --app <appDir> [--request /path ...] [--tools <dir>]
                            N cold runs (fresh workerd each) of the main module's import minus
                            an empty-import floor = app startup; first requests on the real
                            entry; startup CPU by module (profile); what executed at startup
                            (precise coverage from inside the isolate); the leads. --pin
                            makes this the baseline. The app dir, the request set and
                            --tools are remembered; later \`worker-scan\` runs bare.
  worker-measure            timing + first requests only
  worker-attrib             startup CPU by EVALUATING module, from the attribution build
                            (ROLLDOWN_PERF_KIT_ATTRIB=1 ... build): charges library work at
                            startup to the module whose top level runs it, with the import
                            chain. The bundled profile cannot (no line ticks for chunk
                            top-level code; ESM evaluation is not a call stack).
  rolldown-install --app <appDir> [--from <packages/rolldown>]
                            copy a built rolldown package (dist with its native binding) over
                            the copy the app's vite resolves; the default source is the
                            rolldown package next to this one (an installed perf-kit has none:
                            pass --from a checkout built with \`just build-rolldown-release\`).
                            The npm release has no report mode, so without this no build
                            writes the report; worker-config says when that is the case. The
                            swap is not a config change. Rebuild and re-pin afterwards.
                            \`<package manager> install --force\` restores the release.
  --request <path>          a route to hit after startup on the real entry; the first hit
                            of a route that imports a deferred module pays that load, and
                            the report prints it next to the startup gain (a deferral MOVES
                            work, it does not delete it). Repeatable. \`ws:/path\` opens a
                            WebSocket instead, \`ws:/path|<text>\` also sends <text>; timed
                            to the server's last message (agent chats and RPC run there).
  --tools <dir>             a project whose wrangler is ${COVERAGE_MIN_WRANGLER} or later, used for the
                            coverage run only (the in-process inspector session landed in
                            workerd 1.20260706.1). Timing and the profile keep the app's own workerd.
  graph | what-if | cut --root | why   work on the Worker's report (node_modules/.rolldown/
                            report/dist/<worker>) once worker-scan remembered the target.
  Requirements, the coverage difference per mode, and the rules: AGENTS.md.

the loop:
  1. build the app; scan --app <appDir> (first scan pins the baseline)
  2. read EVERY signal in the scan output: render-blocking CSS gate + render gap (fix
     these FIRST - inline critical CSS / render with bundled defaults instead of awaiting
     fetches / un-hide a hero that an entry animation mounts at opacity 0: LCP counts
     the first visibly-painted frame), pre-paint CPU by module, static pre-paint
     transfer (fetched before paint, executed after - make it load on demand), cold
     bytes at paint (fetched+parsed before paint but mostly unread - a
     partially-executed vendor SDK usually hides one boot-time init call), defer
     candidates, large modules "executed" at paint (data evaluates on import -
     executed is not needed), sibling variant groups (locales/themes: load only the
     active one), statically retained imports (rolldown builds: the module graph
     prices every split candidate - if the verdict says the graph is not collected,
     enable it, it is one config line)
  3. read the app source; find why the landing page pays for each finding. On rolldown
     devtools report builds, \`graph\` + \`what-if <module>\` answer this statically: the
     exact import chain (via/idom) and the bytes a deferral frees; \`cut <module>\` gives the
     fewest import edits to detach a multi-path module - no chain-tracing by hand
  4. change the app (never remove features); one change at a time
  5. rebuild; run the app's functional check; scan (--quick to probe, full scan to decide)
  6. "improvement beyond noise" + check passes -> keep, scan --pin (or baseline), commit;
     otherwise revert + rebuild
  7. repeat. When you have finished optimizing: scan --parity. A difference starts
     recover mode - often the build was faster BECAUSE it broke (a style that stopped
     applying, a hero swapped for a placeholder); fix or revert until it matches.
     Declare done ONLY when the verdict reports every signal class clear -
     never because one report looks empty (a tool's silence is not "done").
     Stopping earlier is allowed ONLY with the verdict checklist copied into your
     summary and every OPEN lead justified (sub-noise measurement or concrete constraint)

judge only by "vs pinned baseline".
${CLI.startsWith('npx ') ? `state for this directory: ${STATE_DIR} (keep it out of commits)\n` : ''}full contract: ${path.join(ROOT, 'AGENTS.md')}
demo-app details: ${path.join(ROOT, 'README.md')}
(read them at those exact paths - never search the filesystem for them: a
\`find / -iname ...\` grinds a full CPU core for hours on a large machine)`);
}

const [command, ...rest] = process.argv.slice(2);
const commands = {
  help: cmdHelp,
  scan: cmdScan,
  gen: cmdGen,
  build: cmdBuild,
  measure: cmdMeasure,
  coverage: cmdCoverage,
  profile: cmdProfile,
  verdict: cmdVerdict,
  parity: cmdParity,
  graph: cmdGraph,
  'what-if': cmdWhatIf,
  cut: cmdCut,
  'graph-diff': cmdGraphDiff,
  baseline: cmdBaseline,
  target: cmdTarget,
  'node-scan': cmdNodeScan,
  'node-measure': cmdNodeMeasure,
  'worker-scan': cmdWorkerScan,
  'worker-measure': cmdWorkerMeasure,
  'worker-attrib': cmdWorkerAttrib,
  'worker-config': cmdWorkerConfig,
  'rolldown-install': cmdRolldownInstall,
  why: cmdWhy,
  field: cmdField,
  record: cmdRecord,
  replay: cmdReplay,
  defer: (argv) => cmdDefer(argv, 'deferred'),
  undefer: (argv) => cmdDefer(argv, 'baseline'),
  status: cmdStatus,
  serve: cmdServe,
};

if (!command || !commands[command]) {
  await cmdHelp();
  process.exit(command ? 2 : 0);
}
try {
  await commands[command](rest);
} catch (err) {
  console.error(`error: ${err.message}`);
  process.exit(1);
}
