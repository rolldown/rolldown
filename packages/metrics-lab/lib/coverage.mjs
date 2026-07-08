// First-paint vs settled V8 precise coverage for the entry chunk, attributed to
// source modules through the chunk's sourcemap (hand-rolled VLQ decode, no deps).
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

// --- browser driver ----------------------------------------------------------

/**
 * One coverage-instrumented navigation. Arms the profiler on a same-origin blank
 * page (so the renderer process that will run index.html is already instrumented),
 * snapshots at first paint (25ms poll) and again after load + readiness + settle.
 */
export async function coverageRun(cdp, {
  origin,
  throttle,
  expectedFeatures = [],
  entryName = '/main.js',
  settleMs = 2000,
  timeoutMs = 60_000,
}) {
  const page = await openPage(cdp, { throttle, injectScript: OBSERVER_JS });
  try {
    await page.navigate(`${origin}/blank.html`);
    await sleep(150);
    await page.send('Profiler.enable');
    await page.send('Profiler.startPreciseCoverage', { callCount: false, detailed: true });
    await page.navigate(`${origin}/index.html`);

    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const fcp = await page.evaluate('window.__perfMetrics ? window.__perfMetrics.fcp : null');
      if (fcp != null) break;
      if (Date.now() > deadline) throw new Error('coverage run: FCP never fired');
      await sleep(25);
    }
    const atPaint = await page.send('Profiler.takePreciseCoverage');

    for (;;) {
      const done = await page.evaluate(`(() => {
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

    return {
      atPaint: entryFunctions(atPaint, entryName),
      atSettle: entryFunctions(atSettle, entryName),
    };
  } finally {
    await page.close().catch(() => {});
  }
}

function entryFunctions(result, entryName) {
  const script = result.result.find((s) => s.url.endsWith(entryName));
  if (!script) {
    const seen = result.result.map((s) => s.url).filter(Boolean).join(', ') || '(none)';
    throw new Error(`coverage: entry script ${entryName} not seen (scripts: ${seen})`);
  }
  return script.functions;
}
