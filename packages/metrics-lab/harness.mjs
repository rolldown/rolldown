#!/usr/bin/env node
// Browser-loading perf harness — the measurement + mutation primitives an agent
// drives to run the optimize loop (see README.md for the loop protocol).
// Prototype of metrics-plan Phase 3b (lab runner) + 3c (coverage).
//
//   node harness.mjs gen [--force]        generate the demo app
//   node harness.mjs build                build it (rolldown, devtools metrics mode)
//   node harness.mjs measure [...]        N throttled runs -> runtime-metrics.json (+delta/baselineDelta)
//   node harness.mjs coverage [...]       first-paint vs settled coverage -> candidates
//   node harness.mjs baseline             pin current runtime + build state as fixed baseline
//   node harness.mjs defer <feature>      lazy-load one feature (rebuild afterwards)
//   node harness.mjs undefer <feature>    revert a defer (rebuild afterwards)
//   node harness.mjs status               current feature modes + last numbers
//   node harness.mjs serve [--port N]     serve dist for manual poking

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
  DEFAULT_THROTTLE, deltaSection, summarize, timedRun,
} from './lib/measure.mjs';
import { coverageBySource, coverageRun } from './lib/coverage.mjs';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.join(ROOT, 'app');
const STATE_DIR = path.join(ROOT, 'state');
const RUNTIME_METRICS = path.join(STATE_DIR, 'runtime-metrics.json');
const RUNTIME_STATE = path.join(STATE_DIR, '.state.json');
const RUNTIME_BASELINE = path.join(STATE_DIR, 'baseline.json');
const COVERAGE_JSON = path.join(STATE_DIR, 'coverage.json');
const HISTORY = path.join(STATE_DIR, 'history.jsonl');
const BUILD_METRICS_DIR = path.join(STATE_DIR, 'rolldown-metrics');
const PROFILE_DIR = path.join(STATE_DIR, 'chrome-profile');

// Decision thresholds the runbook references. The harness only REPORTS against
// them; accepting or reverting a change is the loop driver's (agent's) call.
const NOISE_FLOOR_MS = 30;
const NOISE_FLOOR_PCT = 2;
const CANDIDATE_MIN_BYTES = 3 * 1024;
const CANDIDATE_MAX_PAINT_RATIO = 0.02;

const kb = (n) => `${(n / 1024).toFixed(1)}KB`;
const ms = (v) => (v == null ? 'n/a' : `${Math.round(v)}ms`);
const readJson = (file) => (fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8')) : null);
const writeJson = (file, value) => fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);

const cleanups = [];
process.on('SIGINT', async () => {
  for (const fn of cleanups.splice(0)) await fn().catch(() => {});
  process.exit(130);
});

function parse(argv, options) {
  return parseArgs({ args: argv, options, allowPositionals: true }).values;
}

async function withServerAndBrowser(distDir, throttleOff, fn) {
  if (!fs.existsSync(path.join(distDir, 'index.html'))) {
    throw new Error(`no build at ${distDir} - run \`node harness.mjs build\` first`);
  }
  const server = await startServer(distDir);
  const browser = await launchBrowser({ profileDir: PROFILE_DIR });
  cleanups.push(server.close, browser.close);
  try {
    const throttle = throttleOff ? null : DEFAULT_THROTTLE;
    return await fn({ origin: server.origin, cdp: browser.cdp, throttle });
  } finally {
    await browser.close().catch(() => {});
    await server.close().catch(() => {});
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
  fs.mkdirSync(STATE_DIR, { recursive: true });
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
    runs: { type: 'string', default: '5' },
    warmup: { type: 'string', default: '1' },
    label: { type: 'string', default: '' },
    settle: { type: 'string', default: '1500' },
    'no-throttle': { type: 'boolean', default: false },
    dist: { type: 'string' },
    features: { type: 'string' },
  });
  const distDir = opts.dist ? path.resolve(opts.dist) : path.join(APP_DIR, 'dist');
  const expectedFeatures = opts.features !== undefined
    ? opts.features.split(',').filter(Boolean)
    : (opts.dist ? [] : FEATURE_NAMES);
  const runs = Number(opts.runs);
  const warmup = Number(opts.warmup);
  const settleMs = Number(opts.settle);
  fs.mkdirSync(STATE_DIR, { recursive: true });

  const samples = await withServerAndBrowser(distDir, opts['no-throttle'], async ({ origin, cdp, throttle }) => {
    const url = `${origin}/index.html`;
    const collected = [];
    for (let i = 0; i < warmup; i++) {
      process.stderr.write(`warmup ${i + 1}/${warmup}...\n`);
      await timedRun(cdp, { url, throttle, expectedFeatures, settleMs });
    }
    for (let i = 0; i < runs; i++) {
      const sample = await timedRun(cdp, { url, throttle, expectedFeatures, settleMs });
      collected.push(sample);
      process.stderr.write(`run ${i + 1}/${runs}: LCP ${ms(sample.lcp)}, load ${ms(sample.load)}\n`);
    }
    return collected;
  });

  const summary = summarize(samples, expectedFeatures);
  const prev = readJson(RUNTIME_STATE);
  const baseline = readJson(RUNTIME_BASELINE);
  const report = {
    schemaVersion: 1,
    generatedAtMs: Date.now(),
    label: opts.label || null,
    throttle: opts['no-throttle'] ? null : DEFAULT_THROTTLE,
    deferred: opts.dist ? null : deferredList(),
    runs: summary.runs,
    metrics: summary.metrics,
    guard: summary.guard,
    delta: prev ? deltaSection(prev.metrics, summary.metrics) : null,
    baselineDelta: baseline ? deltaSection(baseline.metrics, summary.metrics) : null,
    samples: summary.samples,
  };
  writeJson(RUNTIME_METRICS, report);
  writeJson(RUNTIME_STATE, {
    schemaVersion: 1, tsMs: report.generatedAtMs, label: report.label, metrics: report.metrics,
  });
  fs.appendFileSync(HISTORY, `${JSON.stringify({
    tsMs: report.generatedAtMs,
    label: report.label,
    deferred: report.deferred,
    metrics: report.metrics,
    guard: report.guard,
  })}\n`);

  printMeasureSummary(report, baseline);
  console.log(`\nfull report: ${RUNTIME_METRICS}`);
}

function deferredList() {
  const modes = featureModes(APP_DIR);
  return modes ? FEATURE_NAMES.filter((n) => modes[n] === 'deferred') : [];
}

function printMeasureSummary(report, baseline) {
  const m = report.metrics;
  console.log(`\n${report.label ? `[${report.label}] ` : ''}${report.runs} runs`
    + `${report.deferred?.length ? `, deferred: ${report.deferred.join(', ')}` : ''}`);
  console.log(`LCP ${ms(m['runtime.lcp_ms'])} (p75 ${ms(m['runtime.lcp_p75_ms'])}) | `
    + `FCP ${ms(m['runtime.fcp_ms'])} | load ${ms(m['runtime.load_ms'])} | `
    + `CLS ${m['runtime.cls']} | transfer ${kb(m['runtime.transfer_bytes'])} | `
    + `${m['runtime.js_request_count']} js requests`);
  const guardOk = report.guard.allFeaturesReady && report.guard.heroRendered && report.guard.lcpObservedInAllRuns;
  console.log(`guard: ${guardOk ? 'PASS' : `FAIL ${JSON.stringify(report.guard)}`}`);

  if (report.baselineDelta?.['runtime.lcp_ms']) {
    const d = report.baselineDelta['runtime.lcp_ms'];
    const threshold = Math.max(NOISE_FLOOR_MS, (NOISE_FLOOR_PCT / 100) * d.prev);
    const call = d.delta <= -threshold ? 'improvement beyond noise'
      : d.delta >= threshold ? 'REGRESSION beyond noise'
        : 'within noise';
    console.log(`vs pinned baseline: LCP ${d.delta > 0 ? '+' : ''}${Math.round(d.delta)}ms `
      + `(${d.pct > 0 ? '+' : ''}${d.pct}%) -> ${call} (noise threshold ${Math.round(threshold)}ms)`);
  } else if (baseline) {
    console.log('vs pinned baseline: n/a (metric missing)');
  } else {
    console.log('no pinned baseline yet - run `node harness.mjs baseline` to pin this measurement');
  }
  if (report.delta?.['runtime.lcp_ms']) {
    const d = report.delta['runtime.lcp_ms'];
    console.log(`vs previous measure: LCP ${d.delta > 0 ? '+' : ''}${Math.round(d.delta)}ms (${d.pct > 0 ? '+' : ''}${d.pct}%)`);
  }
}

async function cmdCoverage(argv) {
  const opts = parse(argv, {
    settle: { type: 'string', default: '2000' },
    'no-throttle': { type: 'boolean', default: false },
    dist: { type: 'string' },
    features: { type: 'string' },
    entry: { type: 'string', default: 'main.js' },
  });
  const distDir = opts.dist ? path.resolve(opts.dist) : path.join(APP_DIR, 'dist');
  const expectedFeatures = opts.features !== undefined
    ? opts.features.split(',').filter(Boolean)
    : (opts.dist ? [] : FEATURE_NAMES);
  const entryFile = path.join(distDir, opts.entry);
  const mapFile = `${entryFile}.map`;
  if (!fs.existsSync(mapFile)) {
    throw new Error(`no sourcemap at ${mapFile} - build with sourcemap: true`);
  }
  fs.mkdirSync(STATE_DIR, { recursive: true });

  const cov = await withServerAndBrowser(distDir, opts['no-throttle'], ({ origin, cdp, throttle }) =>
    coverageRun(cdp, {
      origin,
      throttle,
      expectedFeatures,
      entryName: `/${opts.entry.replaceAll('\\', '/')}`,
      settleMs: Number(opts.settle),
    }));

  const code = fs.readFileSync(entryFile, 'utf8');
  const map = JSON.parse(fs.readFileSync(mapFile, 'utf8'));
  const rows = coverageBySource({ code, map, atPaint: cov.atPaint, atSettle: cov.atSettle });

  const modules = [...rows.entries()]
    .map(([source, row]) => ({
      source,
      totalBytes: row.totalBytes,
      paintBytes: row.paintBytes,
      settleBytes: row.settleBytes,
      paintRatio: row.totalBytes ? row.paintBytes / row.totalBytes : 0,
      settleRatio: row.totalBytes ? row.settleBytes / row.totalBytes : 0,
    }))
    .sort((a, b) => b.totalBytes - a.totalBytes);

  // Defer candidates. For the demo app these map to feature marker blocks (the
  // seams `defer <name>` can rewrite); for --dist they are advisory per-module.
  const modes = opts.dist ? null : featureModes(APP_DIR);
  const candidates = [];
  for (const mod of modules) {
    if (mod.totalBytes < CANDIDATE_MIN_BYTES || mod.paintRatio >= CANDIDATE_MAX_PAINT_RATIO) continue;
    const feature = FEATURE_NAMES.find((n) => mod.source.endsWith(`features/${n}.ts`));
    if (modes) {
      if (!feature || modes[feature] !== 'baseline') continue; // already deferred, or not a known seam
      candidates.push({ feature, ...mod });
    } else {
      candidates.push({ feature: feature ?? null, ...mod });
    }
  }

  writeJson(COVERAGE_JSON, {
    schemaVersion: 1,
    generatedAtMs: Date.now(),
    entry: opts.entry,
    deferred: opts.dist ? null : deferredList(),
    thresholds: { candidateMinBytes: CANDIDATE_MIN_BYTES, candidateMaxPaintRatio: CANDIDATE_MAX_PAINT_RATIO },
    modules,
    candidates,
  });

  console.log(`entry-chunk coverage (${opts.entry}, first paint vs settled):\n`);
  const pct = (r) => `${(r * 100).toFixed(1)}%`;
  for (const mod of modules) {
    const verdict = mod.paintRatio >= CANDIDATE_MAX_PAINT_RATIO ? 'used-before-paint'
      : mod.settleRatio >= CANDIDATE_MAX_PAINT_RATIO ? 'post-paint-only'
        : 'not-executed-by-settle';
    console.log(`  ${mod.source.padEnd(34)} ${kb(mod.totalBytes).padStart(9)}  `
      + `paint ${pct(mod.paintRatio).padStart(6)}  settle ${pct(mod.settleRatio).padStart(6)}  ${verdict}`);
  }
  if (candidates.length) {
    console.log(`\ndefer candidates (>=${kb(CANDIDATE_MIN_BYTES)}, <${CANDIDATE_MAX_PAINT_RATIO * 100}% executed at paint), largest first:`);
    for (const c of candidates) {
      console.log(`  ${c.feature ?? c.source}  (${kb(c.totalBytes)})`
        + `${c.feature ? `  -> node harness.mjs defer ${c.feature}` : ''}`);
    }
  } else {
    console.log('\nno defer candidates left at current thresholds.');
  }
  console.log(`\nfull report: ${COVERAGE_JSON}`);
}

async function cmdBaseline() {
  const state = readJson(RUNTIME_STATE);
  if (!state) throw new Error('nothing to pin - run `node harness.mjs measure` first');
  writeJson(RUNTIME_BASELINE, state);
  console.log(`runtime baseline pinned (label: ${state.label ?? 'none'}, LCP ${ms(state.metrics['runtime.lcp_ms'])})`);
  const buildState = path.join(BUILD_METRICS_DIR, '.state.json');
  if (fs.existsSync(buildState)) {
    fs.copyFileSync(buildState, path.join(BUILD_METRICS_DIR, 'baseline.json'));
    console.log('build baseline pinned (rolldown-metrics/baseline.json)');
  }
}

async function cmdDefer(argv, mode) {
  const feature = argv[0];
  if (!feature) throw new Error(`usage: node harness.mjs ${mode === 'deferred' ? 'defer' : 'undefer'} <${FEATURE_NAMES.join('|')}>`);
  const result = setFeatureMode(APP_DIR, feature, mode);
  console.log(`${feature}: ${mode}${result.changed ? '' : ' (already)'}`);
  if (result.changed) console.log('rebuild before measuring: node harness.mjs build');
}

async function cmdStatus() {
  const modes = featureModes(APP_DIR);
  if (!modes) {
    console.log('no app generated yet (node harness.mjs gen)');
    return;
  }
  console.log('feature modes:');
  for (const [name, mode] of Object.entries(modes)) console.log(`  ${name.padEnd(12)} ${mode}`);
  const entry = path.join(APP_DIR, 'dist', 'main.js');
  console.log(`entry chunk: ${fs.existsSync(entry) ? kb(fs.statSync(entry).size) : '(not built)'}`);
  const last = readJson(RUNTIME_STATE);
  if (last) console.log(`last measure: ${last.label ?? '(unlabeled)'} LCP ${ms(last.metrics['runtime.lcp_ms'])}`);
  const baseline = readJson(RUNTIME_BASELINE);
  console.log(baseline
    ? `pinned baseline: ${baseline.label ?? '(unlabeled)'} LCP ${ms(baseline.metrics['runtime.lcp_ms'])}`
    : 'pinned baseline: none');
}

async function cmdServe(argv) {
  const opts = parse(argv, { port: { type: 'string', default: '4646' }, dist: { type: 'string' } });
  const distDir = opts.dist ? path.resolve(opts.dist) : path.join(APP_DIR, 'dist');
  const server = await startServer(distDir, Number(opts.port));
  console.log(`serving ${distDir} at ${server.origin} (Ctrl+C to stop)`);
  await new Promise(() => {});
}

// --- dispatch ----------------------------------------------------------------

const [command, ...rest] = process.argv.slice(2);
const commands = {
  gen: cmdGen,
  build: cmdBuild,
  measure: cmdMeasure,
  coverage: cmdCoverage,
  baseline: cmdBaseline,
  defer: (argv) => cmdDefer(argv, 'deferred'),
  undefer: (argv) => cmdDefer(argv, 'baseline'),
  status: cmdStatus,
  serve: cmdServe,
};

if (!command || !commands[command]) {
  console.error(`usage: node harness.mjs <${Object.keys(commands).join('|')}> [options]\nsee README.md for the loop protocol`);
  process.exit(2);
}
try {
  await commands[command](rest);
} catch (err) {
  console.error(`error: ${err.message}`);
  process.exit(1);
}
