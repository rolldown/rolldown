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
import {
  coverageBySource, coverageRun, largeAtPaintModules, siblingVariantGroups,
} from './lib/coverage.mjs';
import { aggregateProfile, profileRun } from './lib/profile.mjs';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.join(ROOT, 'app');
const STATE_DIR = path.join(ROOT, 'state');
const RUNTIME_METRICS = path.join(STATE_DIR, 'runtime-metrics.json');
const RUNTIME_STATE = path.join(STATE_DIR, '.state.json');
const RUNTIME_BASELINE = path.join(STATE_DIR, 'baseline.json');
const COVERAGE_JSON = path.join(STATE_DIR, 'coverage.json');
const PROFILE_JSON = path.join(STATE_DIR, 'profile.json');
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
    dist: distDir,
    entry: detectEntry(distDir),
    throttle: opts['no-throttle'] ? null : DEFAULT_THROTTLE,
    deferred: opts.dist ? null : deferredList(),
    runs: summary.runs,
    metrics: summary.metrics,
    guard: summary.guard,
    gatingFetches: summary.gatingFetches,
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
  const guardOk = report.guard.allFeaturesReady
    && report.guard.heroRendered !== false // null = no hero probe on this app
    && report.guard.lcpObservedInAllRuns;
  console.log(`guard: ${guardOk ? 'PASS' : `FAIL ${JSON.stringify(report.guard)}`}`);

  // Deep signals: things LCP alone doesn't say, each with the move it suggests.
  const renderGap = m['runtime.render_gap_ms'];
  if (typeof renderGap === 'number' && renderGap > 150) {
    console.log(`render gap: first paint landed ${Math.round(renderGap)}ms AFTER the load event - rendering is gated on post-load work, not on downloading.`);
    for (const fetchLine of report.gatingFetches ?? []) {
      console.log(`  completed just before paint: ${fetchLine}`);
    }
    console.log('  next: find what the boot path awaits before the first render (a config/data fetch, a locale chunk) and render with bundled defaults instead, applying the fetched result when it arrives.');
  }
  const prepaintCpu = m['runtime.prepaint_longtask_ms'];
  if (typeof prepaintCpu === 'number' && prepaintCpu > 150) {
    console.log(`pre-paint CPU: ${Math.round(prepaintCpu)}ms of long tasks before first paint.`);
    console.log('  next: run `node harness.mjs profile --dist <dist>` to see which modules burn that CPU; defer work the first paint does not need. ORDER MATTERS: fix any render gap (above) first - CPU that overlaps a render-blocking fetch is free, so deferring it can measure worse until the fetch is fixed.');
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
      console.log('next: keep this change - re-pin with `node harness.mjs baseline`, then commit it');
    } else {
      console.log('next: this attempt did not clearly improve LCP - revert the change and rebuild (then try a different one)');
    }
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

/** The entry chunk of a built app, read from its index.html module script. */
function detectEntry(distDir) {
  const indexFile = path.join(distDir, 'index.html');
  if (!fs.existsSync(indexFile)) return null;
  const html = fs.readFileSync(indexFile, 'utf8');
  for (const tag of html.match(/<script\b[^>]*>/g) ?? []) {
    if (!tag.includes('type="module"')) continue;
    const src = tag.match(/\bsrc="([^"]+)"/)?.[1];
    if (src && !src.startsWith('http')) return src.replace(/^\.?\//, '');
  }
  return null;
}

async function cmdCoverage(argv) {
  const opts = parse(argv, {
    settle: { type: 'string', default: '2000' },
    'no-throttle': { type: 'boolean', default: false },
    dist: { type: 'string' },
    features: { type: 'string' },
    entry: { type: 'string' },
  });
  const distDir = opts.dist ? path.resolve(opts.dist) : path.join(APP_DIR, 'dist');
  const expectedFeatures = opts.features !== undefined
    ? opts.features.split(',').filter(Boolean)
    : (opts.dist ? [] : FEATURE_NAMES);
  // Hashed asset names (Vite et al.) make a fixed --entry impractical; read the
  // module script from the built index.html unless one was given explicitly.
  const entry = opts.entry ?? detectEntry(distDir) ?? 'main.js';
  if (!opts.entry) console.log(`entry: ${entry} (auto-detected from dist/index.html)`);
  const entryFile = path.join(distDir, entry);
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
      entryName: `/${entry.replaceAll('\\', '/')}`,
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
    entry,
    deferred: opts.dist ? null : deferredList(),
    thresholds: { candidateMinBytes: CANDIDATE_MIN_BYTES, candidateMaxPaintRatio: CANDIDATE_MAX_PAINT_RATIO },
    modules,
    candidates,
  });

  console.log(`entry-chunk coverage (${entry}, first paint vs settled):\n`);
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
    console.log(opts.dist
      ? '\nnext: change the app so the landing page stops loading these before first paint\n'
        + '(follow their import chains from the entry), rebuild, run its functional check, then measure.'
      : '\nnext: defer the top candidate, rebuild, then measure.');
  } else {
    console.log('\nno defer candidates at current thresholds - nothing sizeable is parsed-but-unexecuted.');
  }

  // Executed-at-paint is NOT the same as needed-at-paint: top-level data counts
  // as "executed" the moment its module is imported. Surface the places where
  // that inversion typically hides real weight.
  const largeHot = largeAtPaintModules(modules);
  if (largeHot.length) {
    console.log('\nlarge modules fully executed at paint - "executed" does NOT prove the first paint needs their contents (top-level data evaluates on import):');
    for (const mod of largeHot) {
      console.log(`  ${mod.source}  (${kb(mod.totalBytes)}, ${(mod.paintRatio * 100).toFixed(0)}% at paint)`);
    }
    console.log('next: for each, check how much of it the first render actually reads; split rarely-read parts (full records, long bodies, alternate variants) into a module reached only by dynamic import.');
  }

  for (const group of siblingVariantGroups(modules)) {
    console.log(`\nsibling group ${group.dir}: ${group.files} modules, ${kb(group.bytes)}, ~${Math.round((group.paintBytes / group.bytes) * 100)}% executed at paint.`);
    console.log('next: families of same-shaped modules (locales, themes, per-tenant configs) usually need only ONE variant per session - keep the default in the entry and load the active variant with a dynamic import.');
  }
  console.log(`\nfull report: ${COVERAGE_JSON}`);
}

async function cmdProfile(argv) {
  const opts = parse(argv, {
    'no-throttle': { type: 'boolean', default: false },
    dist: { type: 'string' },
    entry: { type: 'string' },
  });
  const distDir = opts.dist ? path.resolve(opts.dist) : path.join(APP_DIR, 'dist');
  const entry = opts.entry ?? detectEntry(distDir) ?? 'main.js';
  if (!opts.entry) console.log(`entry: ${entry} (auto-detected from dist/index.html)`);
  const entryFile = path.join(distDir, entry);
  const mapFile = `${entryFile}.map`;
  if (!fs.existsSync(mapFile)) {
    throw new Error(`no sourcemap at ${mapFile} - build with sourcemap: true`);
  }
  fs.mkdirSync(STATE_DIR, { recursive: true });

  const profile = await withServerAndBrowser(distDir, opts['no-throttle'], ({ origin, cdp, throttle }) =>
    profileRun(cdp, { origin, throttle }));

  const { rows, totalMs } = aggregateProfile(profile, {
    code: fs.readFileSync(entryFile, 'utf8'),
    map: JSON.parse(fs.readFileSync(mapFile, 'utf8')),
    entryUrlSuffix: `/${entry.replaceAll('\\', '/')}`,
  });
  writeJson(path.join(STATE_DIR, 'profile.json'), {
    schemaVersion: 1, generatedAtMs: Date.now(), entry, totalMs, rows,
  });

  console.log(`boot CPU by module, navigation -> first paint (${totalMs}ms sampled):\n`);
  for (const row of rows.slice(0, 20)) {
    const pct = totalMs > 0 ? `${((row.ms / totalMs) * 100).toFixed(0).padStart(4)}%` : '';
    console.log(`  ${row.bucket.padEnd(40)} ${String(row.ms).padStart(7)}ms ${pct}`);
  }
  console.log('\nnext: work here runs BEFORE the page paints. Defer whatever the first render does not need (idle callback + dynamic import). Fix render-gating fetches first: CPU that overlaps a blocked render is free, so deferring it can measure worse until the fetch is fixed.');
  console.log(`\nfull report: ${path.join(STATE_DIR, 'profile.json')}`);
}

// The lesson behind this command: a diagnostic tool's completeness claim is
// load-bearing — an agent that trusts a premature "converged" stops in front of
// real wins. `verdict` therefore fuses EVERY signal class, refuses to conclude
// while any lead is open or any signal is missing/stale, and states the tools'
// blind-spot boundary even when everything is clear.
async function cmdVerdict(argv) {
  const opts = parse(argv, { dist: { type: 'string' } });
  const distDir = opts.dist ? path.resolve(opts.dist) : path.join(APP_DIR, 'dist');
  const distFlag = opts.dist ? ` --dist ${opts.dist}` : '';
  const entry = detectEntry(distDir) ?? 'main.js';
  const entryFile = path.join(distDir, entry);
  if (!fs.existsSync(entryFile)) {
    throw new Error(`no build at ${distDir} - build the app first`);
  }
  const builtAtMs = fs.statSync(entryFile).mtimeMs;
  // A report vouches only for the build it ran against: entry filename must match
  // (hashed names change with content) and it must postdate the current build.
  const fresh = (report) => Boolean(report)
    && (!report.entry || report.entry === entry)
    && report.generatedAtMs >= builtAtMs;

  const runtime = readJson(RUNTIME_METRICS);
  const coverage = readJson(COVERAGE_JSON);
  const profile = readJson(PROFILE_JSON);

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
      `node harness.mjs measure${distFlag} --runs 5 --label <name>`);
  } else {
    const gap = runtime.metrics['runtime.render_gap_ms'];
    if (typeof gap === 'number' && gap > 150) {
      lead('open', `render gap ${Math.round(gap)}ms`,
        `paint is gated on post-load work${runtime.gatingFetches?.length ? ` (${runtime.gatingFetches.join('; ')})` : ''}`,
        'render with bundled defaults and apply fetched results when they arrive - fix this before judging CPU deferrals');
    } else {
      lead('clear', 'render gap', gap == null ? 'not measurable' : `paint lands ${Math.round(gap)}ms after load`);
    }

    const cpu = runtime.metrics['runtime.prepaint_longtask_ms'];
    if (typeof cpu !== 'number' || cpu <= 150) {
      lead('clear', 'pre-paint CPU', cpu == null ? 'no long tasks observed' : `${Math.round(cpu)}ms of long tasks before paint (baseline territory)`);
    } else if (!fresh(profile)) {
      lead('unknown', `pre-paint CPU ${Math.round(cpu)}ms`,
        profile ? 'profile is stale (dist was rebuilt after it)' : 'not yet attributed to modules',
        `node harness.mjs profile${distFlag}`);
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
    lead('unknown', 'coverage (candidates / large-at-paint / sibling groups)',
      coverage ? 'coverage report is stale (dist was rebuilt after it)' : 'no coverage run yet',
      `node harness.mjs coverage${distFlag}`);
  } else {
    const candidates = coverage.candidates ?? [];
    if (candidates.length) {
      lead('open', `defer candidates (${candidates.length})`,
        candidates.slice(0, 3).map((c) => `${c.feature ?? c.source} ${kb(c.totalBytes)}`).join(', '),
        'lazy-load them, rebuild, re-measure');
    } else {
      lead('clear', 'defer candidates', 'nothing sizeable is parsed-but-unexecuted');
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

  console.log(`verdict for ${distDir} (entry ${entry})\n`);
  for (const line of lines) console.log(line);
  console.log('');
  if (openCount + unknownCount === 0) {
    console.log('VERDICT: every signal class is clear and fresh. Nothing further is indicated by these tools.');
    console.log('Boundary: they do not see image/CSS/font weight, server latency, cache policy, or');
    console.log('interaction-time cost. Remaining LCP is baseline network + parse + render for what the');
    console.log('page genuinely needs at first paint.');
  } else {
    console.log(`VERDICT: not done - ${openCount} lead(s) OPEN, ${unknownCount} signal(s) UNKNOWN or stale.`);
    console.log('Work the OPEN items (render gap first), gather the UNKNOWN signals, rebuild, re-measure.');
  }
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

async function cmdHelp() {
  console.log(`browser-loading perf harness - measurement + diagnosis; you drive the loop

commands (this directory):
  measure --dist <app>/dist --runs 5 --label <name>   throttled runs -> LCP, verdict, render-gap + pre-paint CPU flags
  baseline                                            pin the last measurement as the fixed reference
  coverage --dist <app>/dist                          bytes executed at paint -> candidates, large-at-paint data, sibling groups
  profile --dist <app>/dist                           boot CPU by module, navigation -> first paint
  verdict --dist <app>/dist                           fuse all signals -> OPEN/clear/UNKNOWN checklist; the only "done" that counts
  gen | build | defer <f> | undefer <f> | status | serve    demo-app helpers (README.md)

the loop:
  1. build the app; measure --label baseline; baseline (pin BEFORE changing anything)
  2. read EVERY signal, not just the LCP line:
     - measure "render gap": rendering is gated on post-load work (usually an awaited
       fetch - render with bundled defaults instead). FIX THIS CLASS FIRST.
     - measure "pre-paint CPU" -> run profile: modules burning CPU before paint that
       the first render may not need (deferring only pays AFTER the render gap is fixed)
     - coverage sections: defer candidates (parsed, never ran); large modules "executed"
       at paint (data evaluates on import - executed does NOT mean needed); sibling
       variant groups (locales/themes: one variant per session, load it dynamically)
  3. read the app source; find why the landing page pays for each finding
  4. change the app (never remove features); one change at a time
  5. rebuild; run the app's functional check; measure --label <change>
  6. "improvement beyond noise" + check passes -> keep, re-pin (baseline), commit; otherwise revert + rebuild
  7. repeat from 2. Declare done ONLY when the verdict command reports every signal
     class clear - never because one report looks empty (a tool's silence is not "done")

judge only by "vs pinned baseline". full contract: AGENTS.md`);
}

const [command, ...rest] = process.argv.slice(2);
const commands = {
  help: cmdHelp,
  gen: cmdGen,
  build: cmdBuild,
  measure: cmdMeasure,
  coverage: cmdCoverage,
  profile: cmdProfile,
  verdict: cmdVerdict,
  baseline: cmdBaseline,
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
