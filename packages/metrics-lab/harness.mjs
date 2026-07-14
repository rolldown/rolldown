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
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { launchBrowser } from './lib/cdp.mjs';
import { startServer } from './lib/serve.mjs';
import { buildApp } from './lib/build.mjs';
import {
  FEATURES, FEATURE_NAMES, featureModes, generateApp, setFeatureMode,
} from './lib/gen-app.mjs';
import {
  DEFAULT_THROTTLE, deltaSection, heavyPrepaintTypes, summarize, timedRun, weightLabel,
} from './lib/measure.mjs';
import {
  COLD_OPEN_MIN_BYTES, attributeChunks, coldAtPaintModules, coverageRun,
  largeAtPaintModules, siblingVariantGroups,
} from './lib/coverage.mjs';
import { aggregateProfile, profileRun } from './lib/profile.mjs';
import { loadModuleGraph, moduleGraphCandidates, resolveModule, whatIf } from './lib/module-graph.mjs';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.join(ROOT, 'app');
// State placement: inside the package while developing it from the repo, but for
// an installed copy (anywhere under node_modules) state belongs to the USER'S
// project — node_modules is wiped on reinstall and should never hold results.
const STATE_DIR = process.env.METRICS_LAB_STATE
  ?? (/[\\/]node_modules[\\/]/.test(ROOT) ? path.join(process.cwd(), '.metrics-lab') : path.join(ROOT, 'state'));
const TARGET_FILE = path.join(STATE_DIR, 'target.json');
const BUILD_METRICS_DIR = path.join(STATE_DIR, 'rolldown-metrics');
const CHROME_PROFILE_DIR = path.join(STATE_DIR, 'chrome-profile');

// Decision thresholds the runbook references. The harness only REPORTS against
// them; accepting or reverting a change is the loop driver's (agent's) call.
const NOISE_FLOOR_MS = 30;
const NOISE_FLOOR_PCT = 2;
// Sequential early stop: when a scan has a pinned baseline and run 1's LCP
// delta is at least this many noise thresholds away from it, the remaining
// runs cannot flip the accept/revert call - they only refine a median nobody
// needs, at the price of full page loads (minutes on slow apps). Sample count
// is the SAFE adaptive dimension; the throttle is the objective function and
// never adapts (zero-throttle hides byte weight and RTT waterfalls - the main
// fix classes - and can invert decisions near the floor).
const EARLY_STOP_FACTOR = 5;
const CANDIDATE_MIN_BYTES = 3 * 1024;
const CANDIDATE_MAX_PAINT_RATIO = 0.02;

// How to spell an invocation in hints: a launcher can dictate it (rolldown-metrics),
// otherwise the bin name when installed, the script when run from the repo.
const CLI = process.env.METRICS_LAB_CLI
  ?? (/[\\/]node_modules[\\/]/.test(ROOT) ? 'npx metrics-lab' : 'node harness.mjs');

const kb = (n) => `${(n / 1024).toFixed(1)}KB`;
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
process.on('SIGINT', async () => {
  for (const fn of cleanups.splice(0)) await fn().catch(() => {});
  process.exit(130);
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

function targetSlug(dist) {
  return dist.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(-60);
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
  };
}

// Resolve --app/<appDir> to an EXISTING build dir, or throw with the fix. A guessed
// path must never be remembered: a haiku agent once obeyed the fabricated path in
// our error message and rewired the app's outDir (then "fixed" the functional check).
function resolveAppDist(appArg) {
  const app = path.resolve(appArg);
  const candidates = ['dist', 'build', 'out'].map((dir) => path.join(app, dir));
  const dist = candidates.find((dir) => fs.existsSync(path.join(dir, 'index.html')))
    // --app aimed at the built dir itself is a common miss - accept it.
    ?? (fs.existsSync(path.join(app, 'index.html')) ? app : null);
  if (!dist) {
    throw new Error(
      `no build found under ${app} - tried dist/, build/, out/, and the dir itself (none has an index.html).\n`
      + 'Build the app first. --app takes the APP ROOT (the dir you build from); if the build output\n'
      + 'lives somewhere else, pass --dist <builtDir> directly. Never change the app\'s outDir to fit\n'
      + 'this tool - aim the tool at the build instead.',
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
    console.log(`target: ${dist} (remembered - future commands can omit --app/--dist)`);
  }
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
      `no build at ${target.dist} - build the app first.\n`
      + '(Wrong target? --app <appRoot> resolves dist/build/out; --dist <builtDir> aims directly.)',
    );
  }
  const entry = opts.entry ?? detectEntry(target.dist);
  if (!entry) {
    throw new Error(
      `could not detect the entry script in ${path.join(target.dist, 'index.html')} - `
      + 'pass --entry <file> (path relative to the build dir).',
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
    locals.push({ src: src.split('?')[0].replace(/^\.?\//, ''), module: tag.includes('type="module"') });
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

async function withServerAndBrowser(distDir, throttleOff, fn) {
  if (!fs.existsSync(path.join(distDir, 'index.html'))) {
    throw new Error(`no build at ${distDir} - build the app first`);
  }
  const server = await startServer(distDir);
  const browser = await launchBrowser({ profileDir: CHROME_PROFILE_DIR });
  cleanups.push(server.close, browser.close);
  try {
    const throttle = throttleOff ? null : DEFAULT_THROTTLE;
    return await fn({ origin: server.origin, cdp: browser.cdp, throttle });
  } finally {
    await browser.close().catch(() => {});
    await server.close().catch(() => {});
  }
}

// --- measure core ----------------------------------------------------------------

async function gatherSamples(cdp, origin, { throttle, expectedFeatures, runs, warmup, settleMs, earlyStop = null }) {
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
        process.stderr.write(`early stop after run 1: |dLCP| ${Math.round(Math.abs(deltaMs))}ms >= ${earlyStop.factor}x noise threshold ${Math.round(earlyStop.thresholdMs)}ms\n`);
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

function writeMeasureReport(target, samples, { expectedFeatures, label, throttle, earlyStopped = null }) {
  const summary = summarize(samples, expectedFeatures);
  const prev = readJson(target.paths.runtimeState);
  const baseline = readJson(target.paths.runtimeBaseline);
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
    delta: prev ? deltaSection(prev.metrics, summary.metrics) : null,
    baselineDelta: baseline ? deltaSection(baseline.metrics, summary.metrics) : null,
    samples: summary.samples,
  };
  writeJson(target.paths.runtimeMetrics, report);
  // runs travels with the state so pinBaseline can refuse single-run pins
  // (quick probes and early-stopped scans - a 1-run baseline poisons deltas).
  writeJson(target.paths.runtimeState, {
    schemaVersion: 1, tsMs: report.generatedAtMs, label: report.label, runs: report.runs, metrics: report.metrics,
  });
  fs.appendFileSync(target.paths.history, `${JSON.stringify({
    tsMs: report.generatedAtMs,
    label: report.label,
    deferred: report.deferred,
    metrics: report.metrics,
    guard: report.guard,
  })}\n`);
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
function printMeasureSummary(report, hadBaseline, { advice = true } = {}) {
  const m = report.metrics;
  console.log(`\n${report.label ? `[${report.label}] ` : ''}${report.runs} runs`
    + `${report.deferred?.length ? `, deferred: ${report.deferred.join(', ')}` : ''}`);
  if (report.earlyStopped) {
    const e = report.earlyStopped;
    console.log(`early stop after run 1 of ${e.plannedRuns}: |dLCP vs pinned baseline| ${Math.abs(e.deltaMs)}ms >= ${e.factor}x noise threshold ${e.thresholdMs}ms`
      + ' - the remaining runs cannot flip this call. Decision-grade for accept/revert; pinning still needs a full scan.');
  }
  console.log(`LCP ${ms(m['runtime.lcp_ms'])} (p75 ${ms(m['runtime.lcp_p75_ms'])}) | `
    + `FCP ${ms(m['runtime.fcp_ms'])} | load ${ms(m['runtime.load_ms'])} | `
    + `CLS ${m['runtime.cls']} | transfer ${kb(m['runtime.transfer_bytes'])} | `
    + `${m['runtime.js_request_count']} js requests`);
  const guardOk = report.guard.allFeaturesReady
    && report.guard.heroRendered !== false // null = no hero probe on this app
    && report.guard.lcpObservedInAllRuns;
  console.log(`guard: ${guardOk ? 'PASS' : `FAIL ${JSON.stringify(report.guard)}`}`);

  // Deep signals: things LCP alone doesn't say, each with the move it suggests.
  const gate = report.renderBlockingGate;
  if (gate?.gating) {
    console.log(`render-blocking CSS gate: ${gate.count} blocking stylesheet(s), ${gate.kb}KB held ${Math.round(gate.shareOfFcp * 100)}% of the FCP timeline (last finished ${gate.lastEndMs}ms, FCP ${gate.fcpMs}ms).`);
    for (const w of (gate.worst ?? []).slice(0, 3)) {
      console.log(`  blocking until ${w.end}ms: ${w.name} (${kb(w.bytes)})`);
    }
    if (advice) {
      console.log('  next: nothing paints until these finish - inline the small critical CSS, load the rest');
      console.log('  non-blocking (preload + media swap), and split styles only later routes need. Fix this');
      console.log('  and any render-gating fetch before judging CPU/JS deferrals.');
    }
  }
  const renderGap = m['runtime.render_gap_ms'];
  if (typeof renderGap === 'number' && renderGap > 150) {
    console.log(`render gap: first paint landed ${Math.round(renderGap)}ms AFTER the load event - rendering is gated on post-load work, not on downloading.`);
    const fetches = report.gatingFetches ?? [];
    for (const fetchLine of fetches) {
      console.log(`  completed just before paint: ${fetchLine}`);
    }
    const prepaint = (report.resourceWeight ?? []).filter((w) => w.preFcpCount > 0);
    if (prepaint.length) {
      console.log(`  before first paint: ${prepaint.slice(0, 5).map(weightLabel).join(', ')}`);
    }
    const heavy = heavyPrepaintTypes(report.resourceWeight);
    if (!advice) {
      // verdict names the gap's cause class and the move - no second copy here
    } else if (fetches.length) {
      console.log('  next: find what the boot path awaits before the first render (a config/data fetch, a locale chunk) and render with bundled defaults instead, applying the fetched result when it arrives.');
    } else if (heavy.some((w) => w.type === 'font')) {
      console.log('  next: no render-gating fetch - the paint likely waits on fonts. Make first paint depend on at most one (subset) font: preload it, and defer loading/registering the rest until after paint.');
    } else if (heavy.length) {
      console.log('  next: no render-gating fetch - heavy images load before first paint. Lazy-load below-the-fold images and shrink the ones the first view needs.');
    } else {
      console.log(`  next: no render-gating fetch or heavy pre-paint asset - the gap is post-load CPU, late-executing chunks, or a hero revealed by an entry animation. Run \`${CLI} profile\` and defer the attributed app work; if the profile shows only framework/baseline work, check whether the LCP element mounts invisible (opacity-0 fade-in wrapper) - LCP counts the first frame it paints VISIBLE, so render the hero visible immediately and animate only decoration.`);
    }
  }
  const prepaintCpu = m['runtime.prepaint_longtask_ms'];
  if (typeof prepaintCpu === 'number' && prepaintCpu > 150) {
    console.log(`pre-paint CPU: ${Math.round(prepaintCpu)}ms of long tasks before first paint.`);
    if (advice) {
      console.log(`  next: run \`${CLI} profile\` to see which modules burn that CPU; defer work the first paint does not need. ORDER MATTERS: fix any render gap (above) first - CPU that overlaps a render-blocking fetch is free, so deferring it can measure worse until the fetch is fixed.`);
    }
  }

  if (report.baselineDelta?.['runtime.lcp_ms']) {
    const d = report.baselineDelta['runtime.lcp_ms'];
    const threshold = Math.max(NOISE_FLOOR_MS, (NOISE_FLOOR_PCT / 100) * d.prev);
    const call = d.delta <= -threshold ? 'improvement beyond noise'
      : d.delta >= threshold ? 'REGRESSION beyond noise'
        : 'within noise';
    console.log(`vs pinned baseline: LCP ${d.delta > 0 ? '+' : ''}${Math.round(d.delta)}ms `
      + `(${d.pct > 0 ? '+' : ''}${d.pct}%) -> ${call} (noise threshold ${Math.round(threshold)}ms)`);
    if (call === 'improvement beyond noise') {
      console.log(report.earlyStopped
        ? `next: keep this change - re-pin with \`${CLI} scan --pin\` (pins need full sampling), then commit it.`
        : `next: keep this change - re-pin with \`${CLI} baseline\` (or scan --pin), then commit it.`);
      console.log(`Probe your NEXT change with \`${CLI} scan --quick\` (one run, minutes cheaper); run a full scan only to accept/revert or pin.`);
    } else {
      console.log('next: this attempt did not clearly improve LCP - revert the change and rebuild (then try a different one).');
      console.log(`Probe the next attempt with \`${CLI} scan --quick\` first; confirm keepers with a full scan.`);
    }
  } else if (hadBaseline) {
    console.log('vs pinned baseline: n/a (metric missing)');
  }
  if (report.delta?.['runtime.lcp_ms']) {
    const d = report.delta['runtime.lcp_ms'];
    console.log(`vs previous measure: LCP ${d.delta > 0 ? '+' : ''}${Math.round(d.delta)}ms (${d.pct > 0 ? '+' : ''}${d.pct}%)`);
  }
}

function pinBaseline(target) {
  const state = readJson(target.paths.runtimeState);
  if (!state) throw new Error('nothing to pin - measure (or scan) first');
  // A 1-run median (quick probe or early-stopped scan) poisons every later
  // delta. scan --pin always runs full sampling - use it.
  if (state.runs === 1) {
    throw new Error('refusing to pin a single-run measurement - run a full scan first (scan --pin re-measures with full sampling and then pins)');
  }
  writeJson(target.paths.runtimeBaseline, state);
  console.log(`baseline pinned (label: ${state.label ?? 'none'}, LCP ${ms(state.metrics['runtime.lcp_ms'])})`);
  if (target.isDemo) {
    const buildState = path.join(BUILD_METRICS_DIR, '.state.json');
    if (fs.existsSync(buildState)) {
      fs.copyFileSync(buildState, path.join(BUILD_METRICS_DIR, 'baseline.json'));
      console.log('build baseline pinned (rolldown-metrics/baseline.json)');
    }
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
    try { file = decodeURIComponent(file); } catch { /* keep raw */ }
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
    if (mod.totalBytes < CANDIDATE_MIN_BYTES || mod.paintRatio >= CANDIDATE_MAX_PAINT_RATIO) continue;
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
    thresholds: { candidateMinBytes: CANDIDATE_MIN_BYTES, candidateMaxPaintRatio: CANDIDATE_MAX_PAINT_RATIO },
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
  const chunkTag = (mod) => (extraChunks.some((c) => c.file === mod.chunk) ? `  [${mod.chunk}]` : '');
  console.log(`initial-load coverage (${entry}${extraChunks.length ? ` + ${extraChunks.length} pre-paint chunk(s)` : ''}, first paint vs settled):\n`);
  if (extraChunks.length) {
    console.log('  pre-paint chunks - fetched AND executed before first paint, so critical-path transfer:');
    for (const c of extraChunks) {
      console.log(`    ${c.file}  ${kb(c.totalBytes)}  paint ${(c.paintRatio * 100).toFixed(0)}%`);
    }
  }
  for (const s of report.skippedChunks ?? []) {
    if (s.reason === 'no-sourcemap') {
      console.log(`  NOTE: chunk ${s.file} executed before paint but has no sourcemap - its bytes are NOT attributed below`);
    }
  }
  const staticPrepaint = (report.skippedChunks ?? [])
    .filter((s) => s.reason === 'static-prepaint-transfer')
    .sort((a, b) => (b.bytes ?? 0) - (a.bytes ?? 0));
  if (staticPrepaint.length) {
    const totalBytes = staticPrepaint.reduce((sum, s) => sum + (s.bytes ?? 0), 0);
    console.log(`  STATIC PRE-PAINT TRANSFER: ${staticPrepaint.length} chunk(s), ${kb(totalBytes)} - fetched BEFORE first paint (static tags/preloads)`);
    console.log('  but executed only after it. Their download competes with the paint for bandwidth. Largest:');
    for (const s of staticPrepaint.slice(0, 10)) {
      console.log(`    ${s.file}  ${kb(s.bytes ?? 0)}${s.neverExecuted ? '  (never executed by settle at all)' : ''}`);
    }
    if (staticPrepaint.length > 10) console.log(`    ... +${staticPrepaint.length - 10} more in coverage.json`);
    if (advice) {
      console.log('  next: load these on demand (dynamic import / drop them from the initial script tags or preloads)');
      console.log('  so the first paint stops paying for their transfer.');
    }
  }
  const lazyChunks = (report.skippedChunks ?? []).filter((s) => s.reason === 'post-paint');
  if (lazyChunks.length) {
    console.log(`  (${lazyChunks.length} chunk(s) fetched AND first executed after paint - already deferred, not analyzed: ${lazyChunks.map((s) => s.file).join(', ')})`);
  }
  if (extraChunks.length || lazyChunks.length || staticPrepaint.length) console.log('');
  const pct = (r) => `${(r * 100).toFixed(1)}%`;
  const shown = modules.slice(0, compact ? 15 : 60);
  for (const mod of shown) {
    const verdict = mod.paintRatio >= CANDIDATE_MAX_PAINT_RATIO ? 'used-before-paint'
      : mod.settleRatio >= CANDIDATE_MAX_PAINT_RATIO ? 'post-paint-only'
        : 'not-executed-by-settle';
    console.log(`  ${mod.source.padEnd(34)} ${kb(mod.totalBytes).padStart(9)}  `
      + `paint ${pct(mod.paintRatio).padStart(6)}  settle ${pct(mod.settleRatio).padStart(6)}  ${verdict}${chunkTag(mod)}`);
  }
  if (modules.length > shown.length) {
    console.log(`  ... +${modules.length - shown.length} more modules (largest shown) - full list in coverage.json`
      + (compact ? ', full table with scan --full' : ''));
  }
  if (candidates.length) {
    const shownCandidates = candidates.slice(0, 12);
    console.log(`\ndefer candidates (>=${kb(CANDIDATE_MIN_BYTES)}, <${CANDIDATE_MAX_PAINT_RATIO * 100}% executed at paint), largest first:`);
    for (const c of shownCandidates) {
      console.log(`  ${c.feature ?? c.source}  (${kb(c.totalBytes)}${chunkTag(c) ? `, in ${c.chunk}` : ''})`
        + `${c.feature ? `  -> node harness.mjs defer ${c.feature}` : ''}`);
    }
    if (candidates.length > shownCandidates.length) {
      console.log(`  ... +${candidates.length - shownCandidates.length} more in coverage.json`);
    }
    if (advice) {
      console.log(target.isDemo
        ? '\nnext: defer the top candidate, rebuild, then measure.'
        : '\nnext: change the app so the landing page stops loading these before first paint\n'
          + '(follow their import chains from the entry), rebuild, run its functional check, then measure.');
    }
  } else {
    console.log('\nno defer candidates at current thresholds - nothing sizeable is parsed-but-unexecuted.');
  }

  // The unified byte view: candidates catch the never-ran extreme, but a module
  // that PARTIALLY executes at boot (vendor SDK init) matches no other bucket
  // while holding the most recoverable weight. Rank by cold bytes so it can't hide.
  const cold = report.coldAtPaint ?? coldAtPaintModules(modules);
  if (cold.length) {
    const shownCold = cold.slice(0, 12);
    console.log('\ncold bytes at paint - fetched+parsed before first paint but mostly unread by it, coldest first:');
    for (const mod of shownCold) {
      console.log(`  ${kb(mod.coldBytes).padStart(9)} cold  (${kb(mod.totalBytes)} @ ${(mod.paintRatio * 100).toFixed(0)}% at paint)  ${mod.source}`
        + `${chunkTag(mod) ? `  [${mod.chunk}]` : ''}${mod.framework ? '  (framework runtime - import edge rarely movable)' : ''}`);
    }
    if (cold.length > shownCold.length) console.log(`  ... +${cold.length - shownCold.length} more in coverage.json`);
    if (advice) {
      console.log('next: for each non-framework module, find the import edge that pulls it in before paint and move');
      console.log('that edge behind interaction/idle (dynamic import). A middling percentage on a vendor SDK usually');
      console.log('means one boot-time init call drags the whole package - defer the call, not just the import.');
    }
  }

  // Executed-at-paint is NOT the same as needed-at-paint: top-level data counts
  // as "executed" the moment its module is imported. Surface the places where
  // that inversion typically hides real weight.
  const largeHot = largeAtPaintModules(modules);
  if (largeHot.length) {
    console.log('\nlarge modules fully executed at paint - "executed" does NOT prove the first paint needs their contents (top-level data evaluates on import):');
    for (const mod of largeHot) {
      console.log(`  ${mod.source}  (${kb(mod.totalBytes)}, ${(mod.paintRatio * 100).toFixed(0)}% at paint${chunkTag(mod) ? `, in ${mod.chunk}` : ''})`);
    }
    if (advice) {
      console.log('next: for each, check how much of it the first render actually reads; split rarely-read parts (full records, long bodies, alternate variants) into a module reached only by dynamic import.');
    }
  }

  for (const group of siblingVariantGroups(modules)) {
    console.log(`\nsibling group ${group.dir}: ${group.files} modules, ${kb(group.bytes)}, ~${Math.round((group.paintBytes / group.bytes) * 100)}% executed at paint.`);
    if (advice) {
      console.log('next: families of same-shaped modules (locales, themes, per-tenant configs) usually need only ONE variant per session - keep the default in the entry and load the active variant with a dynamic import.');
    }
  }
}

// --- profile core ----------------------------------------------------------------

function buildProfileReport(target, profile, entry) {
  const entryFile = path.join(target.dist, entry);
  const { rows, totalMs } = aggregateProfile(profile, {
    code: fs.readFileSync(entryFile, 'utf8'),
    map: JSON.parse(fs.readFileSync(`${entryFile}.map`, 'utf8')),
    entryUrlSuffix: `/${entry.replaceAll('\\', '/')}`,
  });
  const report = { schemaVersion: 1, generatedAtMs: Date.now(), entry, totalMs, rows };
  writeJson(target.paths.profile, report);
  return report;
}

function printProfileReport(report, { advice = true } = {}) {
  console.log(`boot CPU by module, navigation -> first paint (${report.totalMs}ms sampled):\n`);
  for (const row of report.rows.slice(0, 20)) {
    const pctStr = report.totalMs > 0 ? `${((row.ms / report.totalMs) * 100).toFixed(0).padStart(4)}%` : '';
    console.log(`  ${row.bucket.padEnd(40)} ${String(row.ms).padStart(7)}ms ${pctStr}`);
  }
  if (advice) {
    console.log('\nnext: work here runs BEFORE the page paints. Defer whatever the first render does not need (idle callback + dynamic import). Fix render-gating fetches first: CPU that overlaps a blocked render is free, so deferring it can measure worse until the fetch is fixed.');
  }
}

// --- verdict core ----------------------------------------------------------------
// The lesson behind this command: a diagnostic tool's completeness claim is
// load-bearing — an agent that trusts a premature "converged" stops in front of
// real wins. `verdict` therefore fuses EVERY signal class, refuses to conclude
// while any lead is open or any signal is missing/stale, and states the tools'
// blind-spot boundary even when everything is clear.

function printVerdict(target) {
  const entry = requireEntry(target);
  const entryFile = path.join(target.dist, entry);
  const builtAtMs = fs.statSync(entryFile).mtimeMs;
  // A report vouches only for the build it ran against: entry filename must match
  // (hashed names change with content) and it must postdate the current build.
  const fresh = (report) => Boolean(report)
    && (!report.entry || report.entry === entry)
    && report.generatedAtMs >= builtAtMs;

  const runtime = readJson(target.paths.runtimeMetrics);
  const coverage = readJson(target.paths.coverage);
  const profile = readJson(target.paths.profile);

  const lines = [];
  let openCount = 0;
  let unknownCount = 0;
  const lead = (state, title, detail, next) => {
    if (state === 'open') openCount++;
    if (state === 'unknown') unknownCount++;
    const tag = state === 'open' ? '[OPEN]   ' : state === 'unknown' ? '[UNKNOWN]' : '[clear]  ';
    lines.push(`  ${tag} ${title}${detail ? ` - ${detail}` : ''}`);
    if (next && state !== 'clear') lines.push(`            next: ${next}`);
  };

  if (!fresh(runtime)) {
    lead('unknown', 'render gap / pre-paint CPU',
      runtime ? 'measurement is stale (dist was rebuilt after it)' : 'no measurement yet',
      `${CLI} measure --runs 5 --label <name>  (or scan)`);
  } else {
    const gate = runtime.renderBlockingGate;
    if (gate?.gating) {
      lead('open', `render-blocking CSS gates first paint (${gate.count} stylesheet(s), ${gate.kb}KB)`,
        `held ${Math.round(gate.shareOfFcp * 100)}% of the FCP timeline (finished ${gate.lastEndMs}ms, FCP ${gate.fcpMs}ms) - nothing painted until it arrived: ${(gate.worst ?? []).slice(0, 2).map((w) => `${w.name} ${kb(w.bytes)}`).join(', ')}`,
        'inline critical CSS, load the rest non-blocking (preload + media swap), split route-only styles - fix this before judging CPU/JS deferrals');
    } else {
      lead('clear', 'render-blocking CSS',
        gate ? `blocking CSS held only ${Math.round((gate.shareOfFcp ?? 0) * 100)}% of the FCP timeline - not the gate` : 'none observed');
    }

    const gap = runtime.metrics['runtime.render_gap_ms'];
    if (typeof gap === 'number' && gap > 150) {
      const fetches = runtime.gatingFetches ?? [];
      const heavy = heavyPrepaintTypes(runtime.resourceWeight);
      if (fetches.length) {
        lead('open', `render gap ${Math.round(gap)}ms`,
          `paint is gated on post-load work (${fetches.join('; ')})`,
          'render with bundled defaults and apply fetched results when they arrive - fix this before judging CPU deferrals');
      } else if (heavy.length) {
        lead('open', `render gap ${Math.round(gap)}ms`,
          `no render-gating fetch; before first paint: ${heavy.map(weightLabel).join(', ')}`,
          heavy.some((w) => w.type === 'font')
            ? 'make first paint depend on at most one (subset) font - preload it, defer the rest until after paint - fix this before judging CPU deferrals'
            : 'lazy-load below-the-fold images and shrink the ones the first view needs - fix this before judging CPU deferrals');
      } else {
        lead('open', `render gap ${Math.round(gap)}ms`,
          'no render-gating fetch or heavy pre-paint asset - post-load CPU, late-executing chunks, or a hero that mounts invisible',
          `${CLI} profile  (or scan), then defer the attributed app work. If the profile shows only framework/baseline work, check for an entry animation that mounts the LCP element at opacity 0 (fade-in wrapper): LCP counts the first VISIBLY painted frame - render the hero visible immediately, animate only decoration - fix this before judging CPU deferrals`);
      }
    } else {
      lead('clear', 'render gap', gap == null ? 'not measurable' : `paint lands ${Math.round(gap)}ms after load`);
    }

    const cpu = runtime.metrics['runtime.prepaint_longtask_ms'];
    if (typeof cpu !== 'number' || cpu <= 150) {
      lead('clear', 'pre-paint CPU', cpu == null ? 'no long tasks observed' : `${Math.round(cpu)}ms of long tasks before paint (baseline territory)`);
    } else if (!fresh(profile)) {
      lead('unknown', `pre-paint CPU ${Math.round(cpu)}ms`,
        profile ? 'profile is stale (dist was rebuilt after it)' : 'not yet attributed to modules',
        `${CLI} profile  (or scan)`);
    } else {
      const appRows = (profile.rows ?? []).filter((row) =>
        row.bucket.includes('/') && !row.bucket.startsWith('(') && row.ms >= 15);
      if (appRows.length) {
        lead('open', `pre-paint CPU ${Math.round(cpu)}ms`,
          `deferrable app work before paint: ${appRows.slice(0, 3).map((row) => `${row.bucket} ${row.ms}ms`).join(', ')}`,
          'defer work the first render does not need (idle callback + dynamic import)');
      } else {
        lead('clear', `pre-paint CPU ${Math.round(cpu)}ms`,
          'profile attributes it to baseline parse/engine work, not deferrable app modules');
      }
    }
  }

  if (!fresh(coverage)) {
    lead('unknown', 'coverage (cold-at-paint / candidates / large-at-paint / sibling groups)',
      coverage ? 'coverage report is stale (dist was rebuilt after it)' : 'no coverage run yet',
      `${CLI} coverage  (or scan)`);
  } else {
    const candidates = coverage.candidates ?? [];
    const inChunk = (c) => (coverage.chunks?.some((ch) => !ch.entry && ch.file === c.chunk) ? ` [${c.chunk}]` : '');
    if (candidates.length) {
      lead('open', `defer candidates (${candidates.length})`,
        candidates.slice(0, 3).map((c) => `${c.feature ?? c.source} ${kb(c.totalBytes)}${inChunk(c)}`).join(', '),
        'lazy-load them, rebuild, re-measure');
    } else {
      lead('clear', 'defer candidates', 'nothing sizeable is parsed-but-unexecuted');
    }

    const cold = (coverage.coldAtPaint ?? coldAtPaintModules(coverage.modules ?? []))
      .filter((m) => !m.framework);
    const coldOpen = cold.filter((m) => m.coldBytes >= COLD_OPEN_MIN_BYTES);
    if (coldOpen.length) {
      lead('open', `cold bytes at paint (${coldOpen.length} module(s) >=${kb(COLD_OPEN_MIN_BYTES)} cold)`,
        coldOpen.slice(0, 4).map((m) => `${m.source} ${kb(m.coldBytes)} cold of ${kb(m.totalBytes)} @ ${(m.paintRatio * 100).toFixed(0)}%${inChunk(m)}`).join(', '),
        'move their import edges behind interaction/idle; a partially-executed vendor SDK usually hides one boot-time init call - defer the call, not just the import');
    } else {
      lead('clear', 'cold bytes at paint', `no non-framework module holds >=${kb(COLD_OPEN_MIN_BYTES)} unread at paint`);
    }
    const unattributed = (coverage.skippedChunks ?? []).filter((s) => s.reason === 'no-sourcemap');
    if (unattributed.length) {
      lead('unknown', `unattributed pre-paint chunk(s): ${unattributed.map((s) => s.file).join(', ')}`,
        'executed before first paint but built without a sourcemap - their bytes are invisible to coverage',
        'rebuild with sourcemaps for these chunks, then re-run coverage');
    }

    const staticPre = (coverage.skippedChunks ?? [])
      .filter((s) => s.reason === 'static-prepaint-transfer')
      .sort((a, b) => (b.bytes ?? 0) - (a.bytes ?? 0));
    const staticPreBytes = staticPre.reduce((sum, s) => sum + (s.bytes ?? 0), 0);
    if (staticPreBytes >= 100 * 1024) {
      lead('open', `static pre-paint transfer (${staticPre.length} chunk(s), ${kb(staticPreBytes)})`,
        `fetched before paint, executed after it - the paint paid for the download: ${staticPre.slice(0, 3).map((s) => `${s.file} ${kb(s.bytes ?? 0)}`).join(', ')}`,
        'make these load on demand (dynamic import / drop from initial script tags or preloads) - biggest lever when transfer dominates LCP');
    } else {
      lead('clear', 'static pre-paint transfer', staticPre.length ? `only ${kb(staticPreBytes)} fetched-but-unused before paint` : 'nothing fetched before paint that the paint does not use');
    }

    const largeHot = largeAtPaintModules(coverage.modules ?? []);
    if (largeHot.length) {
      lead('open', `large modules executed at paint (${largeHot.length})`,
        largeHot.slice(0, 3).map((m) => `${m.source} ${kb(m.totalBytes)}`).join(', '),
        'executed does not mean needed - verify how much the first render reads; split rarely-read data behind dynamic import');
    } else {
      lead('clear', 'large modules executed at paint', 'none at current thresholds');
    }

    const groups = siblingVariantGroups(coverage.modules ?? []);
    if (groups.length) {
      lead('open', `sibling variant groups (${groups.length})`,
        groups.map((g) => `${g.dir} ${g.files} modules ${kb(g.bytes)}`).join(', '),
        'keep the default variant in the entry, load the active one dynamically');
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
      lead('open', `statically retained imports (${rows.length} module(s) >=${kb(GRAPH_RETAINED_OPEN_BYTES)})`,
        rows.slice(0, 3).map((m) => `${m.id} retains ${kb(m.retainedBytes)}/${m.retainedModuleCount} module(s)${m.idom != null ? ` via ${mods[m.idom].id}` : ''}`).join(', '),
        `${CLI} what-if <module> prices the deferral (exact modules+bytes freed; --keep a,b holds needed parts eager); make the importer use dynamic import(). Retained is potential, not proof - if the first render genuinely needs it, justify with a measurement or constraint`);
    } else {
      lead('clear', 'statically retained imports', `no non-framework module retains >=${kb(GRAPH_RETAINED_OPEN_BYTES)} behind a cuttable static edge`);
    }
  } else if (mg.state === 'stale') {
    lead('unknown', 'static module graph', 'module-graph.json predates the current build - a build without the devtools flag leaves it stale',
      'keep build.rolldownOptions.devtools = { mode: "metrics" } in the vite config, rebuild, re-scan');
  } else if (mg.state === 'absent-rolldown') {
    lead('unknown', 'static module graph', 'not collected - this app builds with rolldown, so one config line enables static split-candidate ranking',
      `vite >= 8: add build.rolldownOptions.devtools = { mode: "metrics" } to the vite config, rebuild, re-scan - \`${CLI} graph\` then ranks every candidate by the bytes a deferral frees, \`${CLI} what-if\` prices one cut, no browser run needed`);
  }

  console.log(`verdict for ${target.dist} (entry ${entry})\n`);
  for (const line of lines) console.log(line);
  console.log('');
  if (fresh(runtime) && runtime.earlyStopped) {
    console.log(`NOTE: the latest measurement EARLY-STOPPED after run 1 - its LCP delta was >=${runtime.earlyStopped.factor}x the`);
    console.log('noise threshold, so the accept/revert call is decision-grade. Pinning still needs a full scan (scan --pin).\n');
  } else if (fresh(runtime) && runtime.runs === 1) {
    console.log('NOTE: the latest measurement used a SINGLE run (quick mode) - its delta is indicative only.');
    console.log('Confirm any accept/revert decision with a full scan (>=3 runs) before acting on it.\n');
  }
  if (openCount + unknownCount === 0) {
    console.log('VERDICT: every signal class is clear and fresh. Nothing further is indicated by these tools.');
    console.log('Boundary: coverage attributes the entry + same-origin pre-paint chunks with sourcemaps;');
    console.log('cross-origin scripts are unattributed, and non-JS weight (fonts/images/CSS) is counted');
    console.log('by type but not analyzed further. Server latency, cache policy, and interaction-time');
    console.log('cost are out of scope. Remaining LCP is baseline network + parse + render for what the');
    console.log('page genuinely needs at first paint.');
  } else {
    console.log(`VERDICT: not done - ${openCount} lead(s) OPEN, ${unknownCount} signal(s) UNKNOWN or stale.`);
    console.log('Work the OPEN items (render gap first), gather the UNKNOWN signals, rebuild, re-measure.');
    console.log('');
    console.log('Do NOT report this work as finished or "confirmed by the harness" while leads are OPEN -');
    console.log('a re-pinned baseline records your gain; it does not close the checklist above.');
    console.log('Copy the checklist verbatim into your final summary. If you stop now, your summary must');
    console.log(`say "stopping with ${openCount} lead(s) OPEN" and justify each one with a measurement`);
    console.log('(you tried it and the delta was sub-noise) or a concrete constraint (framework dep,');
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
  const result = await buildApp({ appDir: APP_DIR, metricsDir: BUILD_METRICS_DIR });
  console.log(`built in ${result.wallMs}ms`);
  console.log(`entry main.js: ${kb(result.entryBytes)}`);
  for (const chunk of result.chunks) console.log(`async ${chunk.file}: ${kb(chunk.bytes)}`);
  const m = result.buildMetrics?.metrics;
  if (m) {
    console.log(`build metrics: total ${m['build.total_ms']}ms, output ${kb(m['output.total_bytes'] ?? 0)}, `
      + `max initial load ${kb(m['output.max_initial_load_bytes'] ?? 0)}`);
    console.log(`build report: ${path.join(BUILD_METRICS_DIR, 'metrics.json')}`);
  } else {
    console.log('build metrics report missing (devtools metrics mode did not run?)');
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
  });
  const target = resolveTarget(opts);
  const expectedFeatures = expectedFeaturesFor(target, opts);
  const pinnedLcp = readJson(target.paths.runtimeBaseline)?.metrics?.['runtime.lcp_ms'];
  const earlyStop = (!opts.pin && !opts['no-early-stop'] && typeof pinnedLcp === 'number')
    ? {
      baselineLcp: pinnedLcp,
      thresholdMs: Math.max(NOISE_FLOOR_MS, (NOISE_FLOOR_PCT / 100) * pinnedLcp),
      factor: EARLY_STOP_FACTOR,
    }
    : null;
  const { samples, earlyStopped } = await withServerAndBrowser(target.dist, opts['no-throttle'], ({ origin, cdp, throttle }) =>
    gatherSamples(cdp, origin, {
      throttle,
      expectedFeatures,
      runs: Number(opts.runs),
      warmup: Number(opts.warmup),
      settleMs: Number(opts.settle),
      earlyStop,
    }));
  const { report, hadBaseline } = writeMeasureReport(target, samples, {
    expectedFeatures,
    label: opts.label,
    throttle: opts['no-throttle'] ? null : DEFAULT_THROTTLE,
    earlyStopped,
  });
  printMeasureSummary(report, hadBaseline);
  if (opts.pin) pinBaseline(target);
  else if (!hadBaseline) console.log(`no pinned baseline yet - run \`${CLI} baseline\` (or pass --pin) to pin this measurement`);
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
    throw new Error(`entry ${entry} has no sourcemap (${path.join(target.dist, entry)}.map) - build with sourcemap: true`);
  }
  const cov = await withServerAndBrowser(target.dist, opts['no-throttle'], ({ origin, cdp, throttle }) =>
    coverageRun(cdp, {
      origin,
      throttle,
      expectedFeatures,
      entryName: `/${entry.replaceAll('\\', '/')}`,
      settleMs: Number(opts.settle),
    }));
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
  if (!fs.existsSync(path.join(target.dist, `${entry}.map`))) {
    throw new Error(`entry ${entry} has no sourcemap (${path.join(target.dist, entry)}.map) - build with sourcemap: true`);
  }
  const profile = await withServerAndBrowser(target.dist, opts['no-throttle'], ({ origin, cdp, throttle }) =>
    profileRun(cdp, { origin, throttle }));
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
    // Disable the sequential early stop (run 1 decides when its LCP delta vs
    // the pinned baseline is >=5x the noise threshold; see EARLY_STOP_FACTOR).
    'no-early-stop': { type: 'boolean', default: false },
  });
  if (opts.quick && opts.pin) {
    throw new Error('scan --quick cannot --pin: a 1-run baseline poisons every later delta. Run a full scan to pin.');
  }
  if (opts.quick) {
    opts.runs = '1';
  }
  const target = resolveTarget(opts);
  const expectedFeatures = expectedFeaturesFor(target, opts);
  const entry = requireEntry(target, opts);
  if (!fs.existsSync(path.join(target.dist, `${entry}.map`))) {
    throw new Error(`entry ${entry} has no sourcemap (${path.join(target.dist, entry)}.map) - build with sourcemap: true`);
  }
  // Early stop needs a decision context (a pinned baseline) and full-median
  // exemptions: --pin measurements BECOME the baseline, so they always sample
  // fully; quick mode is already a single run.
  const pinnedLcp = readJson(target.paths.runtimeBaseline)?.metrics?.['runtime.lcp_ms'];
  const earlyStop = (!opts.quick && !opts.pin && !opts['no-early-stop'] && typeof pinnedLcp === 'number')
    ? {
      baselineLcp: pinnedLcp,
      thresholdMs: Math.max(NOISE_FLOOR_MS, (NOISE_FLOOR_PCT / 100) * pinnedLcp),
      factor: EARLY_STOP_FACTOR,
    }
    : null;

  const gathered = await withServerAndBrowser(target.dist, opts['no-throttle'], async ({ origin, cdp, throttle }) => {
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
    if (opts.quick) return { samples, earlyStopped, cov, profile: null };
    // The profile only ever matters when pre-paint CPU is above the verdict's
    // 150ms threshold - skip its navigation otherwise (a scan-time minute on
    // slow apps). --profile forces it.
    const prepaintMs = summarize(samples, expectedFeatures).metrics['runtime.prepaint_longtask_ms'];
    if (!opts.profile && !(typeof prepaintMs === 'number' && prepaintMs > 150)) {
      process.stderr.write(`profile skipped (pre-paint CPU ${prepaintMs == null ? 'n/a' : `${Math.round(prepaintMs)}ms`} - baseline territory; force with --profile)\n`);
      return { samples, earlyStopped, cov, profile: null };
    }
    process.stderr.write('profile run...\n');
    const profile = await profileRun(cdp, { origin, throttle });
    return { samples, earlyStopped, cov, profile };
  });

  const { report, hadBaseline } = writeMeasureReport(target, gathered.samples, {
    expectedFeatures,
    label: opts.label,
    throttle: opts['no-throttle'] ? null : DEFAULT_THROTTLE,
    earlyStopped: gathered.earlyStopped,
  });
  // Read before buildCoverageReport overwrites it: an existing coverage report
  // means this is a re-scan, so the module table prints compact (--full restores it).
  const isRescan = fs.existsSync(target.paths.coverage);
  const coverageReport = buildCoverageReport(target, gathered.cov, entry);
  const profileReport = gathered.profile ? buildProfileReport(target, gathered.profile, entry) : null;

  // advice:false throughout - the verdict printed below carries every next: line,
  // so the sections above it report numbers without a second copy of the coaching.
  printMeasureSummary(report, hadBaseline, { advice: false });
  console.log('');
  printCoverageReport(target, coverageReport, { compact: isRescan && !opts.full, advice: false });
  console.log('');
  if (profileReport) {
    printProfileReport(profileReport, { advice: false });
    console.log('');
  }
  printGraphSection(target, fs.statSync(path.join(target.dist, entry)).mtimeMs);
  printVerdict(target);

  if (opts.pin) {
    pinBaseline(target);
  } else if (!hadBaseline) {
    if (opts.quick) {
      console.log('(no pinned baseline yet, and quick scans are never pinned - run a full scan to establish it)');
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
  const opts = parse(argv, { ...TARGET_OPTS });
  printVerdict(resolveTarget(opts));
}

// --- module-graph analysis (rolldown devtools metrics builds) ---------------------

// Scan/verdict integration thresholds: a non-framework module retaining >=100KB on
// the initial load is always worth pricing with what-if; the 30s slack absorbs the
// moments between the graph write (generate stage) and the dist flush of one build.
const GRAPH_RETAINED_OPEN_BYTES = 100 * 1024;
const GRAPH_FRESH_SLACK_MS = 30 * 1000;
const GRAPH_FRAMEWORK_RE = /^(react-dom|react|scheduler|vue|@vue|svelte|preact)(\/|$)/;

function moduleGraphStatus(target, builtAtMs = null) {
  const graph = loadModuleGraph(moduleGraphCandidates({
    demoMetricsDir: target.isDemo ? BUILD_METRICS_DIR : null,
    dist: target.dist,
  }));
  if (graph) {
    const stale = typeof builtAtMs === 'number'
      && fs.statSync(graph.file).mtimeMs < builtAtMs - GRAPH_FRESH_SLACK_MS;
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
      return vitePkg.name === 'rolldown-vite'
        || Number(String(vitePkg.version ?? '').split('.')[0]) >= 8;
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
  const isHtmlBootChild = (m) => m.idom != null
    && entrySet.has(mods[m.idom].id) && /\.html?$/i.test(mods[m.idom].id);
  return mods
    .filter((m) => m.staticReachable && !entrySet.has(m.id)
      && m.retainedBytes >= GRAPH_RETAINED_OPEN_BYTES
      && !isHtmlBootChild(m)
      && !GRAPH_FRAMEWORK_RE.test(m.id))
    .sort((a, b) => b.retainedBytes - a.retainedBytes || a.id.localeCompare(b.id));
}

function printGraphSection(target, builtAtMs = null) {
  const mg = moduleGraphStatus(target, builtAtMs);
  if (mg.state === 'absent') return;
  if (mg.state === 'stale') {
    console.log('static module graph: STALE (predates the current build) - rebuild with build.rolldownOptions.devtools = { mode: "metrics" } in the vite config\n');
    return;
  }
  if (mg.state === 'absent-rolldown') {
    console.log(`static module graph: not collected - this app builds with rolldown. vite >= 8: add build.rolldownOptions.devtools = { mode: "metrics" } to the vite config, rebuild, re-scan (unlocks \`${CLI} graph\` / \`${CLI} what-if\`)\n`);
    return;
  }
  const mods = mg.graph.modules;
  const entrySet = new Set(mg.graph.entryModules);
  const rows = mods
    .filter((m) => m.staticReachable && m.retainedBytes > 0 && !entrySet.has(m.id))
    .sort((a, b) => b.retainedBytes - a.retainedBytes || a.id.localeCompare(b.id))
    .slice(0, 8);
  if (!rows.length) return;
  console.log('statically retained imports (rolldown module graph - what one deferral would free):');
  for (const mod of rows) {
    console.log(`  ${kb(mod.retainedBytes).padStart(10)}  ${mod.id}${mod.idom != null ? `  via ${mods[mod.idom].id}` : ''}`);
  }
  console.log(`  full ranking: \`${CLI} graph\`; exact modules+bytes for one cut: \`${CLI} what-if <module>\`\n`);
}

function requireModuleGraph(opts) {
  const target = resolveTarget(opts);
  const graph = loadModuleGraph(moduleGraphCandidates({
    reportDir: opts.report,
    demoMetricsDir: target.isDemo ? BUILD_METRICS_DIR : null,
    dist: target.dist,
  }));
  if (!graph) {
    throw new Error(
      'no module-graph.json found - it is written by rolldown devtools metrics builds.\n'
      + 'vite >= 8: add `build.rolldownOptions.devtools = { mode: "metrics" }` to the vite config\n'
      + '(report lands in node_modules/.rolldown/metrics), rebuild, then re-run.\n'
      + 'Or point at an existing report with --report <dir>.',
    );
  }
  return graph;
}

async function cmdGraph(argv) {
  const opts = parse(argv, { ...TARGET_OPTS, report: { type: 'string' }, top: { type: 'string', default: '15' } });
  const graph = requireModuleGraph(opts);
  const mods = graph.modules;
  console.log(`module graph: ${graph.file}`);
  console.log(`entries: ${graph.entryModules.join(', ')}`);
  const staticMods = mods.filter((m) => m.staticReachable);
  const staticBytes = staticMods.reduce((sum, m) => sum + m.bytes, 0);
  const lazyCount = mods.filter((m) => m.dynamicOnly).length;
  console.log(`initial-load view: ${staticMods.length} modules / ${kb(staticBytes)} statically reachable; ${lazyCount} already lazy (dynamic-import only)\n`);
  const entrySet = new Set(graph.entryModules);
  const rows = staticMods
    .filter((m) => m.retainedBytes > 0 && !entrySet.has(m.id))
    .sort((a, b) => b.retainedBytes - a.retainedBytes || a.id.localeCompare(b.id))
    .slice(0, Number(opts.top));
  console.log('retained size - what deferring each module\'s import edge would remove from the initial load:');
  for (const mod of rows) {
    const via = mod.idom != null ? `  via ${mods[mod.idom].id}` : '  (directly under the entries)';
    console.log(`  ${kb(mod.retainedBytes).padStart(10)}  ${mod.id}  (own ${kb(mod.bytes)}, ${mod.retainedModuleCount} module(s))${via}`);
  }
  if (!rows.length) console.log('  (nothing sizeable is uniquely retained - the entries themselves hold the bytes)');
  console.log(`\nnext: ${CLI} what-if <module>  - the exact modules+bytes that one deferral frees (add --keep a,b to hold some imports eager)`);
}

async function cmdWhatIf(argv) {
  const { values: opts, positionals } = parseArgs({
    args: argv,
    options: { ...TARGET_OPTS, report: { type: 'string' }, keep: { type: 'string' } },
    allowPositionals: true,
  });
  const query = positionals[0];
  if (!query) throw new Error(`usage: ${CLI} what-if <module> [--keep a,b] [--report <dir>]`);
  const graph = requireModuleGraph(opts);
  const resolve = (q) => {
    const hit = resolveModule(graph, q);
    if (!hit) throw new Error(`no module matches '${q}' (ids are project-relative, e.g. src/router.ts)`);
    if (hit.ambiguous) {
      throw new Error(`'${q}' is ambiguous:\n  ${hit.ambiguous.join('\n  ')}\nuse a longer suffix.`);
    }
    return hit.index;
  };
  const target = resolve(query);
  const keep = (opts.keep ?? '').split(',').filter(Boolean).map(resolve);
  const result = whatIf(graph, target, keep);

  console.log(`what-if deferred: ${result.target.id}`);
  if (result.notStaticallyReachable) {
    console.log(result.alreadyLazy
      ? 'already lazy: every path to this module crosses a dynamic import - it costs the initial load nothing.'
      : 'not reachable from the entries at all in this build.');
    return;
  }
  if (result.cutEdges.length) {
    console.log(`cut ${result.cutEdges.length} static import edge(s), from: ${result.cutEdges.join(', ')}`);
  }
  console.log(`removes ${kb(result.removedBytes)} / ${result.removedCount} module(s) from the initial load${keep.length ? ` (keeping ${keep.length} sentry module(s) eager)` : ''}:`);
  const shown = result.removed.slice(0, 20);
  for (const mod of shown) {
    console.log(`  ${kb(mod.bytes).padStart(10)}  ${mod.id}`);
  }
  if (result.removed.length > shown.length) {
    console.log(`  ... +${result.removed.length - shown.length} more`);
  }
  console.log(`\nnext: make those importer(s) load it with a dynamic import(), rebuild, run the app's functional check, then \`${CLI} scan\` to confirm the LCP effect.`);
}

async function cmdBaseline(argv) {
  const opts = parse(argv, { ...TARGET_OPTS });
  pinBaseline(resolveTarget(opts));
}

async function cmdTarget(argv) {
  const opts = parse(argv, { demo: { type: 'boolean', default: false } });
  const positional = argv.filter((a) => !a.startsWith('--'));
  if (opts.demo) {
    fs.rmSync(TARGET_FILE, { force: true });
    console.log('target cleared - commands now default to the demo app');
    return;
  }
  if (positional[0]) {
    const dist = resolveAppDist(positional[0]);
    writeJson(TARGET_FILE, { dist, setAtMs: Date.now() });
    console.log(`target: ${dist}`);
    return;
  }
  const sticky = readJson(TARGET_FILE);
  console.log(sticky?.dist ? `target: ${sticky.dist}` : 'no target set - commands default to the demo app (set one: node harness.mjs target <appDir>)');
}

async function cmdDefer(argv, mode) {
  const feature = argv[0];
  if (!feature) throw new Error(`usage: node harness.mjs ${mode === 'deferred' ? 'defer' : 'undefer'} <${FEATURE_NAMES.join('|')}>`);
  const result = setFeatureMode(APP_DIR, feature, mode);
  console.log(`${feature}: ${mode}${result.changed ? '' : ' (already)'}`);
  if (result.changed) console.log('rebuild before measuring: node harness.mjs build');
}

async function cmdStatus() {
  const sticky = readJson(TARGET_FILE);
  console.log(sticky?.dist ? `target: ${sticky.dist}` : 'target: (none - demo app)');
  const target = resolveTarget({});
  const entryName = detectEntry(target.dist);
  console.log(`entry chunk: ${entryName && fs.existsSync(path.join(target.dist, entryName))
    ? `${entryName} ${kb(fs.statSync(path.join(target.dist, entryName)).size)}`
    : '(not built)'}`);
  const last = readJson(target.paths.runtimeState);
  if (last) console.log(`last measure: ${last.label ?? '(unlabeled)'} LCP ${ms(last.metrics['runtime.lcp_ms'])}`);
  const baseline = readJson(target.paths.runtimeBaseline);
  console.log(baseline
    ? `pinned baseline: ${baseline.label ?? '(unlabeled)'} LCP ${ms(baseline.metrics['runtime.lcp_ms'])}`
    : 'pinned baseline: none');
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
                            >=5x the noise threshold (later runs cannot flip that call;
                            pins always sample fully) - this flag forces all runs
  verdict                   fuse the gathered signals -> OPEN/clear/UNKNOWN; the only "done" that counts

individual commands (same target rules):
  measure [--runs 5] [--label x] [--pin]    timed runs only -> LCP + "vs pinned baseline" verdict
  coverage | profile                        one signal each
  graph                                     STATIC split candidates ranked by retained size (what
                                            deferring each module removes from the initial load) -
                                            needs a rolldown devtools-metrics build (vite >= 8:
                                            build.rolldownOptions.devtools = { mode: "metrics" })
  what-if <module> [--keep a,b]             exact modules+bytes one deferral frees; sentries stay eager
  baseline                                  pin the last measurement as the fixed reference
  target [<appDir>] [--demo]                show / set / clear the remembered target
  gen | build | defer <f> | undefer <f> | status | serve    demo-app helpers (README.md)

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
     devtools-metrics builds, \`graph\` + \`what-if <module>\` answer this statically: the
     exact import chain (via/idom) and the bytes a deferral frees - no chain-tracing by hand
  4. change the app (never remove features); one change at a time
  5. rebuild; run the app's functional check; scan (--quick to probe, full scan to decide)
  6. "improvement beyond noise" + check passes -> keep, scan --pin (or baseline), commit;
     otherwise revert + rebuild
  7. repeat. Declare done ONLY when the verdict reports every signal class clear -
     never because one report looks empty (a tool's silence is not "done").
     Stopping earlier is allowed ONLY with the verdict checklist copied into your
     summary and every OPEN lead justified (sub-noise measurement or concrete constraint)

judge only by "vs pinned baseline".
full contract: ${path.join(ROOT, 'AGENTS.md')}
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
  graph: cmdGraph,
  'what-if': cmdWhatIf,
  baseline: cmdBaseline,
  target: cmdTarget,
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
