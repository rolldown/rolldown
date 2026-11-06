// `field` - measure a DEPLOYED site in the user's own logged-in Chrome and write
// the lab's report files (runtime-metrics.json, coverage.json, profile.json) per
// variant, plus field-report.json with the paired comparison.
//
// What field mode adds to the lab: real data, real network, real server time.
// What it gives up: control. Results are for one account, one network and one
// machine; noise is handled with alternating pairs, retries for invalid runs,
// outlier flags and a bootstrap interval, not hidden. The parity check does not
// apply (real data changes between loads).

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { parseArgs } from 'node:util';

import { sleep } from '../cdp.mjs';
import { attributeChunks, coldAtPaintModules } from '../coverage.mjs';
import { DEFAULT_THROTTLE, median, scaleThrottle, summarize } from '../measure.mjs';
import { aggregateProfile } from '../profile.mjs';
import {
  chunkCoverage,
  criticalInitiatorUrl,
  criticalPath,
  initiatorChain,
  makeRedactor,
  outlierIndexes,
  pairedDelta,
  round,
} from './analysis.mjs';
import { attachBrowser, contextForOrigin, restoreCookies, snapshotCookies } from './browser.mjs';
import { indexBuild, matchScript, matchSummary, readMatched } from './build-match.mjs';
import { describeConfig, loadFieldConfig } from './config.mjs';
import { diagnosticRun, timingRun } from './capture.mjs';
import { onInterrupt } from '../interrupt.mjs';

// Mirrors the lab's defer-candidate thresholds (harness.mjs) so field coverage
// reports read the same way.
const CANDIDATE_MIN_BYTES = 3 * 1024;
const CANDIDATE_MAX_PAINT_RATIO = 0.02;
export const MAX_RETRIES = 3;

const OPTIONS = {
  config: { type: 'string' },
  runs: { type: 'string', default: '6' },
  diagnose: { type: 'string', default: '1' },
  cut: { type: 'string' },
  out: { type: 'string' },
  only: { type: 'string' },
  pause: { type: 'string', default: '3000' },
  settle: { type: 'string', default: '3000' },
  timeout: { type: 'string', default: '30000' },
  throttle: { type: 'string', default: 'none' },
  'net-scale': { type: 'string', default: '1' },
  'cdp-url': { type: 'string' },
  'cdp-port': { type: 'string' },
};

export const slug = (url) =>
  url
    .toLowerCase()
    .replace(/^https?:\/\//, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 60);
export const writeJson = (file, value) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
};
export const msStr = (v) => (typeof v === 'number' ? `${Math.round(v)}ms` : 'n/a');
const mb = (n) => `${(n / 1048576).toFixed(2)}MB`;
const shortUrl = (url, origin) => (url?.startsWith(origin) ? url.slice(origin.length) || '/' : url);

export async function cmdField(argv) {
  const opts = parseArgs({ args: argv, options: OPTIONS, allowPositionals: true }).values;
  if (!opts.config) throw new Error('field needs --config <file> (see README.md, "Field mode")');
  const config = loadFieldConfig(opts.config);
  const redact = makeRedactor(config.redact);
  let variants = Object.values(config.variants);
  if (opts.only) {
    const keep = new Set(opts.only.split(','));
    variants = variants.filter((v) => keep.has(v.name));
  }
  const runs = Number(opts.runs);
  const diagnose = Number(opts.diagnose);
  const cut = opts.cut ?? (config.gate ? 'gate' : 'lcp');
  if (!['fcp', 'lcp', 'gate'].includes(cut)) throw new Error('--cut must be fcp, lcp or gate');
  if (cut === 'gate' && !config.gate) throw new Error('--cut gate needs "gate" in the config');
  const throttle =
    opts.throttle === 'lab' ? scaleThrottle(DEFAULT_THROTTLE, Number(opts['net-scale'])) : null;
  const outDir = path.resolve(
    opts.out ?? path.join(os.tmpdir(), 'perf-kit-field', slug(redact(config.url))),
  );
  const timing = { settleMs: Number(opts.settle), timeoutMs: Number(opts.timeout), throttle };
  const pause = Number(opts.pause);

  console.log(`field: ${redact(config.url)}  variants: ${variants.map((v) => v.name).join(', ')}`);
  console.log(
    `output: ${outDir} (outside the repo; contains no cookies, no page text, no screenshots)`,
  );
  if (throttle) console.log(`throttle: lab x${throttle.netScale} ADDED ON TOP of the real network`);

  const browser = await attachBrowser({ cdpUrl: opts['cdp-url'], cdpPort: opts['cdp-port'] });
  const { cdp } = browser;
  const ctx = await contextForOrigin(cdp, config.origin);
  if (!ctx.found) {
    console.log(
      `note: no open tab on ${config.origin}; using the default profile. Open a logged-in tab first to measure that profile.`,
    );
  }
  const names = [...new Set(variants.flatMap((v) => v.cookies.map((c) => c.name)))];
  const saved = await snapshotCookies(cdp, {
    browserContextId: ctx.browserContextId,
    origin: config.origin,
    names,
  });
  const configured = variants.flatMap((v) => v.cookies);
  // One restore, shared: a Ctrl-C during the final restore waits for it.
  let restoring = null;
  const restore = () =>
    (restoring ??= restoreCookies(cdp, {
      browserContextId: ctx.browserContextId,
      origin: config.origin,
      saved,
      configured,
    }));
  const offInterrupt = onInterrupt(restore);
  const common = { config, context: ctx.browserContextId, saved, configured, redact };
  // Requests in the timing runs carry redacted URLs; a pinned gate matches in that form.
  const gate = redact(config.gate);

  try {
    // --- timing runs: alternating order, pair i = one run of each variant ---------
    const timed = [];
    const invalid = [];
    for (let i = 0; i < runs; i++) {
      const order = i % 2 ? [...variants].reverse() : variants;
      for (const variant of order) {
        for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
          const run = await timingRun(cdp, { ...common, variant, ...timing });
          if (run.invalid) {
            invalid.push({ pair: i + 1, variant: variant.name, reason: run.invalid });
            console.log(
              `  pair ${i + 1} ${variant.name.padEnd(10)} INVALID: ${run.invalid}${attempt < MAX_RETRIES ? ' - retrying' : ''}`,
            );
            await sleep(pause);
            continue;
          }
          const cp = criticalPath(run.requests, run.sample.lcp, { pattern: gate });
          timed.push({ pair: i + 1, variant: variant.name, ...run, criticalPath: cp });
          const b = cp.breakdown;
          console.log(
            `  pair ${i + 1} ${variant.name.padEnd(10)} LCP ${msStr(run.sample.lcp).padStart(7)}` +
              (b
                ? `  gate ${shortUrl(cp.gate.url, config.origin).slice(0, 48)}  start ${msStr(b.beforeGateMs)}  ttfb ${msStr(b.ttfbMs)}  after ${msStr(b.afterGateMs)}`
                : `  gate: ${cp.reason}`),
          );
          await sleep(pause);
          break;
        }
      }
    }

    // --- diagnostic runs: coverage at the cut + settle, CPU profile, bodies ------
    const diagnostics = new Map();
    if (diagnose > 0) {
      console.log(
        `\ndiagnostic runs (coverage cut: ${cut}) - slower by design, not used for timing:`,
      );
      for (const variant of variants) {
        for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
          const run = await diagnosticRun(cdp, { ...common, variant, cut, ...timing });
          if (run.invalid) {
            console.log(
              `  ${variant.name.padEnd(10)} INVALID: ${run.invalid}${attempt < MAX_RETRIES ? ' - retrying' : ''}`,
            );
            await sleep(pause);
            continue;
          }
          diagnostics.set(variant.name, run);
          console.log(
            `  ${variant.name.padEnd(10)} cut at ${msStr(run.cutMs)}, ${run.scripts.length} same-origin scripts`,
          );
          await sleep(pause);
          break;
        }
      }
    }

    // --- reports -------------------------------------------------------------------
    const summaries = {};
    for (const variant of variants) {
      const mine = timed.filter((t) => t.variant === variant.name);
      const runtime = buildRuntimeReport({ variant, mine, invalid, redact, throttle });
      const diag = diagnostics.get(variant.name);
      if (diag && runtime.criticalPath) {
        runtime.criticalPath = {
          ...runtime.criticalPath,
          ...diagnosticChain(runtime.criticalPath, diag, { ...config, gate }),
        };
      }
      if (runtime.criticalPath) delete runtime.criticalPath.medianRunRequests;
      writeJson(path.join(outDir, variant.name, 'runtime-metrics.json'), runtime);
      let coverage = null;
      let profile = null;
      if (diag) {
        ({ coverage, profile } = buildDiagnosticReports({ variant, diag, config, redact }));
        writeJson(path.join(outDir, variant.name, 'coverage.json'), coverage);
        writeJson(path.join(outDir, variant.name, 'profile.json'), profile);
      }
      summaries[variant.name] = { runtime, coverage, profile };
    }
    const report = buildFieldReport({
      variants,
      timed,
      invalid,
      summaries,
      config,
      redact,
      cut,
      throttle,
    });
    writeJson(path.join(outDir, 'field-report.json'), report);
    printFieldReport(report, summaries, config);
    console.log(`\nfiles: ${outDir}`);
  } finally {
    try {
      await restore();
      if (names.length) console.log(`cookies restored: ${names.join(', ')} (values not printed)`);
    } catch (err) {
      console.error(`WARNING: cookie restore failed: ${err.message}`);
    }
    offInterrupt();
    await browser.close().catch(() => {});
  }
}

function criticalSummary(mine) {
  const withPath = mine.filter((t) => t.criticalPath.breakdown);
  if (!withPath.length) return null;
  const pick = (key) =>
    round(
      median(
        withPath.map((t) => t.criticalPath.breakdown[key]).filter((v) => typeof v === 'number'),
      ),
    );
  const gateCounts = new Map();
  for (const t of withPath)
    gateCounts.set(t.criticalPath.gate.url, (gateCounts.get(t.criticalPath.gate.url) ?? 0) + 1);
  const [gateUrl, gateRuns] = [...gateCounts].sort((a, b) => b[1] - a[1])[0];
  const byLcp = [...withPath].sort((a, b) => a.sample.lcp - b.sample.lcp);
  const mid = byLcp[Math.floor(byLcp.length / 2)];
  return {
    gate: { url: gateUrl, runs: gateRuns, of: mine.length },
    medians: {
      documentEndMs: pick('documentEndMs'),
      beforeGateMs: pick('beforeGateMs'),
      gateAfterDocumentMs: pick('gateAfterDocumentMs'),
      ttfbMs: pick('ttfbMs'),
      downloadMs: pick('downloadMs'),
      afterGateMs: pick('afterGateMs'),
    },
    medianRunChain: mid.criticalPath.chain,
    medianRunPair: mid.pair,
    medianRunRequests: mid.requests,
  };
}

/**
 * The chain's SHAPE from the diagnostic run (async stacks name the real caller
 * behind a fetch wrapper), its TIMES from the median timing run (the diagnostic
 * run is slowed by coverage, profiling and the debugger).
 */
export function diagnosticChain(summary, diag, config) {
  const gateReq =
    diag.requests.find(
      (r) => (config.gate ? r.url.includes(config.gate) : r.url === summary.gate.url) && !r.failed,
    ) ?? null;
  if (!gateReq)
    return { chainSource: 'median timing run (gate request not seen in the diagnostic run)' };
  const shape = initiatorChain(diag.requests, criticalInitiatorUrl(gateReq, diag.requests));
  const timingRequests = summary.medianRunRequests ?? [];
  return {
    medianRunChain: shape.map((step) => {
      const t = timingRequests.find((r) => r.url === step.url && !r.failed);
      return t
        ? { ...step, startMs: t.startMs, endMs: t.endMs }
        : { ...step, timesFrom: 'diagnostic run' };
    }),
    gateStack: [...new Set((gateReq.initiator.frames ?? []).map((f) => f.url))],
    chainSource: 'shape from the diagnostic run (async stacks), times from the median timing run',
  };
}

export function buildRuntimeReport({ variant, mine, invalid, redact, throttle }) {
  const summary = summarize(mine.map((t) => t.sample));
  const ttfbs = mine.map((t) => t.criticalPath.breakdown?.ttfbMs ?? null);
  const outliers = outlierIndexes(ttfbs).map((i) => mine[i].pair);
  const nonGet = new Map();
  for (const t of mine) {
    for (const r of t.requests) {
      if (['GET', 'HEAD', 'OPTIONS'].includes(r.method) || r.blocked) continue;
      const key = `${r.method} ${r.url.split('?')[0]}`;
      nonGet.set(key, (nonGet.get(key) ?? 0) + 1);
    }
  }
  return {
    schemaVersion: 1,
    mode: 'field',
    generatedAtMs: Date.now(),
    variant: variant.name,
    url: redact(variant.url),
    throttle,
    runs: summary.runs,
    invalidRuns: invalid.filter((x) => x.variant === variant.name),
    metrics: summary.metrics,
    guard: summary.guard,
    gatingFetches: summary.gatingFetches,
    resourceWeight: summary.resourceWeight,
    renderBlockingGate: summary.renderBlockingGate,
    postLoadScripts: summary.postLoadScripts,
    lcpElement: summary.lcpElement,
    criticalPath: criticalSummary(mine),
    serverOutlierPairs: outliers,
    nonGetRequests: [...nonGet].map(([request, count]) => ({ request, count })),
    runsDetail: mine.map((t) => ({
      pair: t.pair,
      path: t.path,
      lcp: t.sample.lcp,
      fcp: t.sample.fcp,
      criticalPath: t.criticalPath,
      requests: t.requests,
    })),
    samples: summary.samples,
  };
}

export function buildDiagnosticReports({ variant, diag, config, redact }) {
  const index = variant.build && fs.existsSync(variant.build) ? indexBuild(variant.build) : null;
  const toFile = (pathname) => {
    try {
      return decodeURIComponent(pathname).replace(/^\/+/, '');
    } catch {
      return pathname.replace(/^\/+/, '');
    }
  };
  const matches = new Map();
  const chunkRows = [];
  const fetchByPath = new Map(diag.scriptFetches.map((f) => [f.pathname, f]));
  for (const script of diag.scripts) {
    const file = toFile(script.pathname);
    const body = diag.bodies.get(script.url);
    if (body == null) continue;
    const match = matchScript(index, { file, body });
    matches.set(file, match);
    const fetch = fetchByPath.get(script.pathname);
    chunkRows.push({
      file: redact(file),
      match: match.status,
      fetchStartMs: fetch?.startMs ?? null,
      fetchEndMs: fetch?.endMs ?? null,
      fetchedBeforeCut: Boolean(fetch?.prePaint),
      ...chunkCoverage({ bytes: body.length, atCut: script.atPaint, atSettle: script.atSettle }),
    });
  }
  chunkRows.sort((a, b) => b.bytes - b.cutBytes - (a.bytes - a.cutBytes));
  const readChunk = (file) => readMatched(index, matches.get(file));
  const prePaintFetches = new Map(
    diag.scriptFetches.filter((f) => f.prePaint).map((f) => [toFile(f.pathname), f.bytes]),
  );
  const attributed = attributeChunks({
    scripts: diag.scripts,
    entryName: diag.entry ?? '/(no module entry)',
    readChunk,
    prePaintFetches,
  });
  // attributeChunks says 'no-sourcemap' for every unmatched chunk; field mode
  // knows the real reason (usually a build of other code or settings).
  const skipped = attributed.skipped.map((s) =>
    s.reason === 'no-sourcemap' && matches.get(s.file) && !matches.get(s.file).local
      ? { ...s, reason: `no-build-match:${matches.get(s.file).status}` }
      : s,
  );
  const candidates = attributed.modules.filter(
    (m) => m.totalBytes >= CANDIDATE_MIN_BYTES && m.paintRatio < CANDIDATE_MAX_PAINT_RATIO,
  );
  const buildMatch = {
    build: variant.build,
    ...matchSummary(chunkRows.map((r) => ({ status: r.match, bytes: r.bytes }))),
  };
  const preCut = chunkRows.filter((r) => r.fetchedBeforeCut || r.cutBytes > 0);
  const coverage = {
    schemaVersion: 1,
    mode: 'field',
    generatedAtMs: Date.now(),
    entry: diag.entry,
    chunks: attributed.chunks,
    skippedChunks: skipped,
    deferred: null,
    thresholds: {
      candidateMinBytes: CANDIDATE_MIN_BYTES,
      candidateMaxPaintRatio: CANDIDATE_MAX_PAINT_RATIO,
    },
    modules: attributed.modules,
    candidates,
    coldAtPaint: coldAtPaintModules(attributed.modules).slice(0, 20),
    field: {
      variant: variant.name,
      path: diag.path,
      cut: diag.cut,
      cutMs: round(diag.cutMs),
      note: '"paint" fields in this file mean "at the cut" (field.cut), not first paint',
      buildMatch,
      preCut: {
        scripts: preCut.length,
        bytes: preCut.reduce((s, r) => s + r.bytes, 0),
        executedBytes: preCut.reduce((s, r) => s + r.cutBytes, 0),
      },
      chunkCoverage: chunkRows,
    },
  };
  const profileSummary = aggregateProfile(diag.profile, { origin: config.origin, readChunk });
  const profile = {
    schemaVersion: 2,
    mode: 'field',
    generatedAtMs: Date.now(),
    entry: diag.entry,
    ...profileSummary,
    rows: profileSummary.rows.map((r) => ({
      ...r,
      bucket: redact(r.bucket),
      chunks: r.chunks.map(redact),
    })),
    field: { variant: variant.name, cut: diag.cut, cutMs: round(diag.cutMs) },
  };
  return { coverage, profile };
}

export function buildFieldReport({
  variants,
  timed,
  invalid,
  summaries,
  config,
  redact,
  cut,
  throttle,
}) {
  const perVariant = {};
  for (const v of variants) {
    const s = summaries[v.name];
    perVariant[v.name] = {
      validRuns: s.runtime.runs,
      invalidRuns: s.runtime.invalidRuns.length,
      lcpMs: s.runtime.metrics['runtime.lcp_ms'],
      fcpMs: s.runtime.metrics['runtime.fcp_ms'],
      ttfbMs: s.runtime.metrics['runtime.ttfb_ms'],
      criticalPath: s.runtime.criticalPath,
      serverOutlierPairs: s.runtime.serverOutlierPairs,
      buildMatch: s.coverage?.field.buildMatch ?? null,
      preCut: s.coverage?.field.preCut ?? null,
    };
  }
  const comparisons = [];
  const [base, ...others] = variants;
  for (const cand of others) {
    const pairsOf = (metric, skip = new Set()) => {
      const rows = [];
      for (let i = 1; i <= Math.max(0, ...timed.map((t) => t.pair)); i++) {
        if (skip.has(i)) continue;
        const a = timed.find((t) => t.pair === i && t.variant === base.name);
        const b = timed.find((t) => t.pair === i && t.variant === cand.name);
        if (a && b) rows.push([metric(a), metric(b)]);
      }
      return rows;
    };
    const lcp = (t) => t.sample.lcp;
    const gateStart = (t) => t.criticalPath.breakdown?.beforeGateMs ?? null;
    const gateAfterDoc = (t) => t.criticalPath.breakdown?.gateAfterDocumentMs ?? null;
    const docEnd = (t) => t.criticalPath.breakdown?.documentEndMs ?? null;
    const afterGate = (t) => t.criticalPath.breakdown?.afterGateMs ?? null;
    const outlierPairs = new Set([
      ...perVariant[base.name].serverOutlierPairs,
      ...perVariant[cand.name].serverOutlierPairs,
    ]);
    comparisons.push({
      base: base.name,
      candidate: cand.name,
      lcp: pairedDelta(pairsOf(lcp)),
      lcpWithoutServerOutliers: outlierPairs.size ? pairedDelta(pairsOf(lcp, outlierPairs)) : null,
      gateStart: pairedDelta(pairsOf(gateStart)),
      gateAfterDocument: pairedDelta(pairsOf(gateAfterDoc)),
      documentEnd: pairedDelta(pairsOf(docEnd)),
      afterGate: pairedDelta(pairsOf(afterGate)),
      excludedPairs: [...outlierPairs],
    });
  }
  return {
    schemaVersion: 1,
    mode: 'field',
    generatedAtMs: Date.now(),
    config: describeConfig(config, redact),
    cut,
    throttle,
    invalidRuns: invalid,
    variants: perVariant,
    comparisons,
    limits: [
      'one account, one network, one machine - not a population (use field RUM for that)',
      'the gate is a heuristic unless "gate" is set in the config',
      'coverage "paint" fields mean "at the cut"; the snapshot is async, so a few ms of later work can land in it',
      'module attribution needs a local build whose chunks match the served bytes (see buildMatch)',
    ],
  };
}

export function printFieldReport(report, summaries, config) {
  const names = Object.keys(report.variants);
  console.log('\nmedians (valid runs):');
  console.log(
    `  ${'variant'.padEnd(12)} ${'LCP'.padStart(7)} ${'doc end'.padStart(8)} ${'gate start'.padStart(11)} ${'after doc'.padStart(10)} ${'gate ttfb'.padStart(10)} ${'after gate'.padStart(11)}  valid`,
  );
  for (const name of names) {
    const v = report.variants[name];
    const m = v.criticalPath?.medians ?? {};
    console.log(
      `  ${name.padEnd(12)} ${msStr(v.lcpMs).padStart(7)} ${msStr(m.documentEndMs).padStart(8)} ${msStr(m.beforeGateMs).padStart(11)} ${msStr(m.gateAfterDocumentMs).padStart(10)} ${msStr(m.ttfbMs).padStart(10)} ${msStr(m.afterGateMs).padStart(11)}  ${v.validRuns}/${v.validRuns + v.invalidRuns}`,
    );
  }
  for (const name of names) {
    const cp = report.variants[name].criticalPath;
    if (cp)
      console.log(
        `  ${name} gate: ${shortUrl(cp.gate.url, config.origin)} (${cp.gate.runs}/${cp.gate.of} runs)`,
      );
  }
  const fmt = (d) =>
    d
      ? `${d.medianDeltaMs > 0 ? '+' : ''}${d.medianDeltaMs}ms [90% ${d.ci90Ms[0]}..${d.ci90Ms[1]}], faster in ${d.candidateFaster}/${d.pairs}`
      : 'n/a';
  for (const c of report.comparisons) {
    console.log(`\n${c.candidate} vs ${c.base} (paired, negative = ${c.candidate} faster):`);
    console.log(`  LCP          ${fmt(c.lcp)}`);
    if (c.lcpWithoutServerOutliers) {
      console.log(
        `  LCP w/o slow-server pairs ${c.excludedPairs.join(',')}: ${fmt(c.lcpWithoutServerOutliers)}`,
      );
    }
    console.log(
      `  document end ${fmt(c.documentEnd)}   (HTML response: server/worker, not the bundle)`,
    );
    console.log(
      `  gate start   ${fmt(c.gateStart)}   (client time before the request LCP waits for)`,
    );
    console.log(
      `  gate - doc   ${fmt(c.gateAfterDocument)}   (the same, measured from the end of the HTML)`,
    );
    console.log(`  after gate   ${fmt(c.afterGate)}   (client time from that response to LCP)`);
  }
  for (const name of names) {
    const cp = report.variants[name].criticalPath;
    if (!cp?.medianRunChain?.length) continue;
    console.log(
      `\n${name}: what the gate request waited for (median run, pair ${cp.medianRunPair}), newest first:`,
    );
    if (cp.chainSource) console.log(`  (${cp.chainSource})`);
    for (const step of cp.medianRunChain) {
      console.log(
        `  ${String(step.type ?? '?').padEnd(9)} ${msStr(step.startMs).padStart(7)} -> ${msStr(step.endMs).padStart(7)}  ${shortUrl(step.url, config.origin)}`,
      );
    }
  }
  for (const name of names) {
    const cov = summaries[name].coverage;
    if (!cov) continue;
    const f = cov.field;
    const bm = f.buildMatch;
    console.log(`\n${name}: coverage at ${f.cut} (${msStr(f.cutMs)}):`);
    console.log(
      `  scripts fetched or run before the cut: ${f.preCut.scripts}, ${mb(f.preCut.bytes)}, executed ${mb(f.preCut.executedBytes)} (${f.preCut.bytes ? Math.round((f.preCut.executedBytes / f.preCut.bytes) * 100) : 0}%)`,
    );
    console.log('  coldest chunks at the cut (bytes not executed by then):');
    for (const row of f.chunkCoverage
      .filter((r) => r.fetchedBeforeCut || r.cutBytes > 0)
      .slice(0, 8)) {
      console.log(
        `    ${mb(row.bytes - row.cutBytes).padStart(7)} cold of ${mb(row.bytes).padStart(7)}  fetch ${msStr(row.fetchStartMs).padStart(7)}  ${row.file}`,
      );
    }
    console.log(
      `  build match: ${bm.matchedScripts}/${bm.scripts} scripts, ${mb(bm.matchedBytes)} of ${mb(bm.bytes)}` +
        `  ${JSON.stringify(bm.byStatus)}${bm.build ? '' : '  (no "build" in the config for this variant)'}`,
    );
    if (cov.coldAtPaint.length) {
      console.log('  coldest modules at the cut:');
      for (const m of cov.coldAtPaint.slice(0, 8)) {
        console.log(
          `    ${mb(m.coldBytes).padStart(7)} cold  ${m.source}${m.framework ? '  (framework)' : ''}`,
        );
      }
    } else if (bm.build) {
      console.log('  module attribution: none - no served chunk matched the local build bytes');
    }
  }
  if (report.invalidRuns.length) {
    console.log(
      `\ninvalid runs (retried): ${report.invalidRuns.length} - ${[...new Set(report.invalidRuns.map((r) => r.reason))].join('; ')}`,
    );
  }
  console.log(
    '\nlimits: one account, one network, one machine. Judge a change by the paired deltas and their intervals.',
  );
}
