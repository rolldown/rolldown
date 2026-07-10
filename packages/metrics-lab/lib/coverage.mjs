// First-paint vs settled V8 precise coverage for the INITIAL LOAD — the entry
// chunk plus every same-origin chunk that executed before first paint (preloaded
// siblings are critical-path transfer too) — attributed to source modules through
// each chunk's sourcemap (hand-rolled VLQ decode, no deps). Chunks that first
// execute after paint are already deferred and are reported, not analyzed.
//
// Two snapshots per run (takePreciseCoverage RESETS counters, so the second one
// covers only the window between the snapshots):
//   atPaint  — taken the moment FCP is observed: bytes that HAD to run to paint.
//   atSettle — bytes executed between first paint and settle.
// Per-module "by settle" usage is the UNION of both snapshots' executed intervals.
// A module with ~0 executed bytes atPaint but real weight is a lazy-load candidate;
// a module hot atPaint is critical-path and must not be deferred.
//
// Known blind spot (inherent to V8 coverage, worth remembering for real apps): a
// module's top level counts as executed when it evaluates, so weight kept in
// top-level literals looks "used" even if nothing reads it before paint. The demo
// app keeps weight inside function bodies so the signal is clean.

import { openPage, sleep } from './cdp.mjs';
import { OBSERVER_JS } from './measure.mjs';

// --- sourcemap VLQ ---------------------------------------------------------

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const B64_LOOKUP = new Map([...B64].map((c, i) => [c, i]));

function decodeVlqSegment(str) {
  const out = [];
  let value = 0;
  let shift = 0;
  for (const ch of str) {
    const digit = B64_LOOKUP.get(ch);
    if (digit === undefined) throw new Error(`bad VLQ char: ${ch}`);
    value += (digit & 31) << shift;
    if (digit & 32) {
      shift += 5;
    } else {
      const negate = value & 1;
      value >>>= 1;
      out.push(negate ? -value : value);
      value = 0;
      shift = 0;
    }
  }
  return out;
}

/**
 * Flatten sourcemap mappings into contiguous generated-offset spans:
 * [{ start, end, srcIdx }] covering the whole generated file (srcIdx -1 = unmapped).
 * Offsets are UTF-16 units; the generated chunks are ASCII, so they equal bytes.
 */
export function generatedSpans(code, map) {
  const lineStarts = [0];
  for (let i = 0; i < code.length; i++) {
    if (code.charCodeAt(i) === 10) lineStarts.push(i + 1);
  }

  const points = [];
  let srcIdx = 0;
  let srcLine = 0;
  let srcCol = 0;
  let nameIdx = 0;
  const mappingLines = map.mappings.split(';');
  for (let line = 0; line < mappingLines.length; line++) {
    const lineStr = mappingLines[line];
    if (!lineStr) continue;
    let genCol = 0;
    for (const seg of lineStr.split(',')) {
      if (!seg) continue;
      const nums = decodeVlqSegment(seg);
      genCol += nums[0];
      let mapped = -1;
      if (nums.length >= 4) {
        srcIdx += nums[1];
        srcLine += nums[2];
        srcCol += nums[3];
        if (nums.length >= 5) nameIdx += nums[4];
        mapped = srcIdx;
      }
      const lineStart = lineStarts[line];
      if (lineStart === undefined) continue;
      points.push({ start: lineStart + genCol, srcIdx: mapped });
    }
  }
  points.sort((a, b) => a.start - b.start);

  const spans = [];
  if (points.length === 0 || points[0].start > 0) {
    spans.push({ start: 0, end: points.length ? points[0].start : code.length, srcIdx: -1 });
  }
  for (let i = 0; i < points.length; i++) {
    const end = i + 1 < points.length ? points[i + 1].start : code.length;
    if (end > points[i].start) spans.push({ start: points[i].start, end, srcIdx: points[i].srcIdx });
  }
  return spans;
}

// --- coverage range painting ------------------------------------------------

/**
 * V8 block-coverage ranges are properly nested; painting parents before children
 * (start asc, end desc) leaves every offset with its innermost range's count.
 * Returns a piecewise-constant boundary list [{ pos, count }].
 */
export function coverageBounds(functions) {
  const ranges = [];
  for (const fn of functions) {
    for (const r of fn.ranges) ranges.push(r);
  }
  ranges.sort((a, b) => a.startOffset - b.startOffset || b.endOffset - a.endOffset);
  let bounds = [{ pos: 0, count: 0 }];
  for (const r of ranges) bounds = overwrite(bounds, r.startOffset, r.endOffset, r.count);
  return bounds;
}

function overwrite(bounds, start, end, count) {
  if (end <= start) return bounds;
  const out = [];
  let i = 0;
  let current = 0;
  while (i < bounds.length && bounds[i].pos < start) {
    out.push(bounds[i]);
    current = bounds[i].count;
    i++;
  }
  let countAtEnd = current;
  while (i < bounds.length && bounds[i].pos < end) {
    countAtEnd = bounds[i].count;
    i++;
  }
  out.push({ pos: start, count });
  out.push({ pos: end, count: countAtEnd });
  while (i < bounds.length) {
    out.push(bounds[i]);
    i++;
  }
  return out;
}

/** Merge count>0 stretches of a boundary list into sorted [start, end) intervals. */
export function executedIntervals(bounds) {
  const intervals = [];
  for (let i = 0; i < bounds.length; i++) {
    if (bounds[i].count <= 0) continue;
    const end = i + 1 < bounds.length ? bounds[i + 1].pos : bounds[i].pos;
    if (end <= bounds[i].pos) continue;
    const last = intervals[intervals.length - 1];
    if (last && last.end >= bounds[i].pos) last.end = Math.max(last.end, end);
    else intervals.push({ start: bounds[i].pos, end });
  }
  return intervals;
}

function overlapBytes(spans, intervals) {
  const executed = new Array(spans.length).fill(0);
  let j = 0;
  for (let i = 0; i < spans.length; i++) {
    const span = spans[i];
    while (j < intervals.length && intervals[j].end <= span.start) j++;
    for (let k = j; k < intervals.length && intervals[k].start < span.end; k++) {
      executed[i] += Math.min(span.end, intervals[k].end) - Math.max(span.start, intervals[k].start);
      if (intervals[k].end >= span.end) break;
    }
  }
  return executed;
}

/** Union two sorted interval lists into one sorted, non-overlapping list. */
function unionIntervals(a, b) {
  const all = [...a, ...b].sort((x, y) => x.start - y.start);
  const out = [];
  for (const interval of all) {
    const last = out[out.length - 1];
    if (last && last.end >= interval.start) last.end = Math.max(last.end, interval.end);
    else out.push({ ...interval });
  }
  return out;
}

/**
 * Aggregate both snapshots per source module. `settleBytes` is cumulative
 * ("executed at any point by settle"), i.e. the union of the two windows.
 * Returns Map<source, { totalBytes, paintBytes, settleBytes }>.
 */
export function coverageBySource({ code, map, atPaint, atSettle }) {
  const spans = generatedSpans(code, map);
  const paintIntervals = executedIntervals(coverageBounds(atPaint));
  const paintExec = overlapBytes(spans, paintIntervals);
  const settleExec = overlapBytes(
    spans,
    unionIntervals(paintIntervals, executedIntervals(coverageBounds(atSettle))),
  );
  const rows = new Map();
  spans.forEach((span, i) => {
    const source = span.srcIdx >= 0
      ? String(map.sources[span.srcIdx] ?? `#${span.srcIdx}`).replaceAll('\\', '/')
      : '(unmapped)';
    const row = rows.get(source) ?? { totalBytes: 0, paintBytes: 0, settleBytes: 0 };
    row.totalBytes += span.end - span.start;
    row.paintBytes += paintExec[i];
    row.settleBytes += settleExec[i];
    rows.set(source, row);
  });
  return rows;
}

/**
 * Attribute the whole initial load: the entry chunk plus every same-origin JS
 * chunk that executed before first paint. Chunks first executing after paint are
 * already deferred — reported in `skipped` (reason 'post-paint'), not analyzed.
 * `readChunk(file)` returns { code, map } or null when no sourcemap exists
 * (skipped with reason 'no-sourcemap' — never silently).
 */
export function attributeChunks({ scripts, entryName, readChunk }) {
  const chunks = [];
  const modules = [];
  const skipped = [];
  for (const script of scripts) {
    const isEntry = script.pathname.endsWith(entryName);
    // Inline scripts surface under the document URL; only .js files are chunks.
    if (!isEntry && !/\.[mc]?js$/.test(script.pathname)) continue;
    const file = script.pathname.replace(/^\/+/, '');
    if (!isEntry && !script.atPaint) {
      skipped.push({ file, reason: 'post-paint' });
      continue;
    }
    const loaded = readChunk(file);
    if (!loaded) {
      skipped.push({ file, reason: 'no-sourcemap' });
      continue;
    }
    const rows = coverageBySource({
      code: loaded.code,
      map: loaded.map,
      atPaint: script.atPaint ?? [],
      atSettle: script.atSettle ?? [],
    });
    const totals = { totalBytes: 0, paintBytes: 0, settleBytes: 0 };
    for (const [source, row] of rows.entries()) {
      modules.push({
        source,
        chunk: file,
        totalBytes: row.totalBytes,
        paintBytes: row.paintBytes,
        settleBytes: row.settleBytes,
        paintRatio: row.totalBytes ? row.paintBytes / row.totalBytes : 0,
        settleRatio: row.totalBytes ? row.settleBytes / row.totalBytes : 0,
      });
      totals.totalBytes += row.totalBytes;
      totals.paintBytes += row.paintBytes;
      totals.settleBytes += row.settleBytes;
    }
    chunks.push({
      file,
      entry: isEntry,
      ...totals,
      paintRatio: totals.totalBytes ? totals.paintBytes / totals.totalBytes : 0,
    });
  }
  chunks.sort((a, b) => (b.entry ? 1 : 0) - (a.entry ? 1 : 0) || b.totalBytes - a.totalBytes);
  modules.sort((a, b) => b.totalBytes - a.totalBytes);
  return { chunks, modules, skipped };
}

// --- lead analyses over per-module coverage rows -------------------------------

export const LARGE_AT_PAINT_MIN_BYTES = 8 * 1024;
export const SIBLING_MIN_FILES = 3;
export const SIBLING_MIN_BYTES = 6 * 1024;
export const COLD_MIN_BYTES = 12 * 1024;      // per-module floor to appear in the cold list
export const COLD_OPEN_MIN_BYTES = 25 * 1024; // a non-framework module this cold keeps the lead OPEN
// Framework runtimes carry real cold bytes (unreached branches) but their import
// edge can't move — annotate instead of flagging, so agents don't chase them.
const FRAMEWORK_RE = /node_modules\/(react-dom|react|scheduler|vue|@vue\/[^/]+|svelte)\//;

/**
 * Big modules that executed at paint. "Executed" does NOT prove the first paint
 * needs their contents — top-level data evaluates the moment its module is
 * imported — so these are verify-need leads, not certainties.
 */
export function largeAtPaintModules(modules) {
  return modules.filter((mod) =>
    mod.totalBytes >= LARGE_AT_PAINT_MIN_BYTES && mod.paintRatio >= 0.5 && mod.source !== '(unmapped)');
}

/**
 * Cold bytes at paint: totalBytes − paintBytes — weight downloaded and parsed
 * before first paint but not executed by it. This is the unified byte view: the
 * <2% defer candidates are its extreme, but a vendor SDK that PARTIALLY
 * initializes at boot (e.g. firestore executing 31%) matches neither the
 * candidate bucket nor the ≥50% large-at-paint bucket while carrying the most
 * recoverable weight on the page. (excalidraw A/B 2026-07-10: firestore's
 * 158KB cold sat exactly in that blind mid-band and decided the eval.)
 */
export function coldAtPaintModules(modules, { minBytes = COLD_MIN_BYTES } = {}) {
  return modules
    .filter((mod) => mod.source !== '(unmapped)')
    .map((mod) => ({
      ...mod,
      coldBytes: Math.max(0, mod.totalBytes - mod.paintBytes),
      framework: FRAMEWORK_RE.test(mod.source),
    }))
    .filter((mod) => mod.coldBytes >= minBytes)
    .sort((a, b) => b.coldBytes - a.coldBytes);
}

/**
 * Same-shaped sibling families (locales, themes, per-tenant configs) mostly
 * executed at paint. Size uniformity over the sizeable members keeps grab-bag
 * utility directories from tripping this; tiny index/barrel files don't count.
 */
export function siblingVariantGroups(modules) {
  const byDir = new Map();
  for (const mod of modules) {
    const slash = mod.source.lastIndexOf('/');
    if (slash <= 0) continue;
    const dir = mod.source.slice(0, slash + 1);
    const group = byDir.get(dir) ?? { dir, files: 0, bytes: 0, paintBytes: 0, sizes: [] };
    group.files += 1;
    group.bytes += mod.totalBytes;
    group.paintBytes += mod.paintBytes;
    group.sizes.push(mod.totalBytes);
    byDir.set(dir, group);
  }
  return [...byDir.values()].filter((group) => {
    const sizeable = group.sizes.filter((size) => size >= 1024);
    return sizeable.length >= SIBLING_MIN_FILES
      && group.bytes >= SIBLING_MIN_BYTES
      && group.paintBytes / group.bytes >= 0.5
      && Math.max(...sizeable) <= Math.min(...sizeable) * 2;
  });
}

// --- browser driver ----------------------------------------------------------

/**
 * One coverage-instrumented navigation. Arms the profiler on a same-origin blank
 * page, snapshots at first paint (25ms poll) and again after load + readiness +
 * settle. Retries once: an unlucky arming race loses the run, not the command.
 */
export async function coverageRun(cdp, options) {
  try {
    return await coverageAttempt(cdp, options);
  } catch (err) {
    if (!String(err.message).includes('not seen')) throw err;
    return await coverageAttempt(cdp, options);
  }
}

async function coverageAttempt(cdp, {
  origin,
  throttle,
  expectedFeatures = [],
  entryName = '/main.js',
  settleMs = 2000,
  timeoutMs = 60_000,
}) {
  const debug = process.env.COVERAGE_DEBUG
    ? (msg) => console.error(`[coverage] ${msg}`)
    : () => {};
  const page = await openPage(cdp, { throttle, injectScript: OBSERVER_JS });
  try {
    await page.navigate(`${origin}/blank.html`);
    await sleep(150);
    await page.send('Profiler.enable');
    const armed = await page.send('Profiler.startPreciseCoverage', { callCount: false, detailed: true });
    debug(`armed on blank (v8 timestamp ${armed?.timestamp})`);
    await page.navigate(`${origin}/index.html`);
    debug('navigate(index) resolved');

    const deadline = Date.now() + timeoutMs;
    for (;;) {
      // Pathname guard: until the index.html navigation commits, evaluate still
      // runs against the blank warm-up document — never read paint entries there.
      const fcp = await page.evaluate(
        "(location.pathname !== '/blank.html' && window.__perfMetrics) ? window.__perfMetrics.fcp : null",
      );
      if (fcp != null) {
        debug(`FCP=${fcp} href=${await page.evaluate('location.href')}`);
        break;
      }
      if (Date.now() > deadline) throw new Error('coverage run: FCP never fired');
      await sleep(25);
    }
    const atPaint = await page.send('Profiler.takePreciseCoverage');
    debug(`atPaint scripts: ${atPaint.result.map((s) => `${s.url || '(inline)'}#${s.functions.length}`).join(', ') || '(none)'}`);

    for (;;) {
      const done = await page.evaluate(`(() => {
        if (location.pathname === '/blank.html') return false;
        const nav = performance.getEntriesByType('navigation')[0];
        if (!nav || !nav.loadEventEnd) return false;
        const r = window.__ready || {};
        return ${JSON.stringify(expectedFeatures)}.every((f) => r[f] === true);
      })()`);
      if (done) break;
      if (Date.now() > deadline) throw new Error('coverage run: page never became ready');
      await sleep(100);
    }
    await sleep(settleMs);
    const atSettle = await page.send('Profiler.takePreciseCoverage');
    await page.send('Profiler.stopPreciseCoverage').catch(() => {});

    // Each take only reports functions executed since the previous take (counters
    // reset), and V8 omits scripts with no new execution entirely. So any script
    // may be legitimately absent from one snapshot: from atSettle when it runs
    // nothing after first paint, or from atPaint when it first executed later.
    // A snapshot a script is absent from contributes null (≠ empty coverage).
    const byUrl = new Map();
    const collect = (take, key) => {
      for (const script of take.result) {
        if (!script.url || !script.url.startsWith(origin)) continue;
        const pathname = new URL(script.url).pathname;
        const rec = byUrl.get(script.url)
          ?? { url: script.url, pathname, atPaint: null, atSettle: null };
        rec[key] = (rec[key] ?? []).concat(script.functions);
        byUrl.set(script.url, rec);
      }
    };
    collect(atPaint, 'atPaint');
    collect(atSettle, 'atSettle');
    const scripts = [...byUrl.values()];
    // The entry absent from BOTH snapshots means the page never ran it — error.
    if (!scripts.some((script) => script.pathname.endsWith(entryName))) {
      const seen = scripts.map((script) => script.pathname).join(', ') || '(none)';
      throw new Error(`coverage: entry script ${entryName} not seen (scripts: ${seen})`);
    }
    return { scripts };
  } finally {
    await page.close().catch(() => {});
  }
}
