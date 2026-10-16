// Boot CPU profile: sample the renderer from navigation until first paint, then
// attribute self-time to source modules through the entry chunk's sourcemap.
// Answers the question coverage can't: "what RAN before paint, and for how long?"
// — which is exactly the evidence needed to judge whether eager boot work
// (telemetry, warm-up caches, data-module evaluation) belongs on the critical path.

import { openPage, sleep } from './cdp.mjs';
import { generatedSpans } from './coverage.mjs';
import { OBSERVER_JS } from './measure.mjs';

/**
 * One profiled navigation. Arms the sampling profiler on the same-origin blank
 * page, navigates, and stops the moment FCP is observed — so the profile covers
 * pre-paint execution only.
 */
export async function profileRun(cdp, { origin, throttle, timeoutMs = 60_000 }) {
  const page = await openPage(cdp, { throttle, injectScript: OBSERVER_JS });
  try {
    await page.navigate(`${origin}/blank.html`);
    await sleep(150);
    await page.send('Profiler.enable');
    await page.send('Profiler.setSamplingInterval', { interval: 200 }); // microseconds
    await page.send('Profiler.start');
    // '/' not '/index.html': router-strict SPAs 404 on the literal file path.
    await page.navigate(`${origin}/`);

    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const fcp = await page.evaluate(
        "(location.pathname !== '/blank.html' && window.__perfMetrics) ? window.__perfMetrics.fcp : null",
      );
      if (fcp != null) break;
      if (Date.now() > deadline) throw new Error('profile run: FCP never fired');
      await sleep(25);
    }
    const { profile } = await page.send('Profiler.stop');
    return profile;
  } finally {
    await page.close().catch(() => {});
  }
}

/** Locate generated (line, column) positions back to source modules. */
function sourceLocator(code, map) {
  const lineStarts = [0];
  for (let i = 0; i < code.length; i++) {
    if (code.charCodeAt(i) === 10) lineStarts.push(i + 1);
  }
  const spans = generatedSpans(code, map); // sorted, contiguous [{start, end, srcIdx}]
  const sourceOf = (span) =>
    span.srcIdx >= 0
      ? String(map.sources[span.srcIdx] ?? `#${span.srcIdx}`).replaceAll('\\', '/')
      : '(unmapped)';
  const spanIndexAt = (offset) => {
    let lo = 0;
    let hi = spans.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      const span = spans[mid];
      if (offset < span.start) hi = mid - 1;
      else if (offset >= span.end) lo = mid + 1;
      else return mid;
    }
    return -1;
  };
  const locate = (line, column) => {
    const lineStart = lineStarts[line];
    if (lineStart === undefined) return null;
    const index = spanIndexAt(lineStart + column);
    return index < 0 ? '(unmapped)' : sourceOf(spans[index]);
  };
  // A whole generated line (1-based, as `positionTicks` reports it) resolves to the
  // source that covers most of it. `positionTicks` carries no column, and a chunk's
  // top-level frame reports no usable `callFrame` position at all (-1:-1 in workerd,
  // 0:0 in Chrome), so this is the only way to attribute module-evaluation time.
  const locateLine = (line1) => {
    const start = lineStarts[line1 - 1];
    if (start === undefined) return null;
    const end = lineStarts[line1] ?? code.length;
    let index = spanIndexAt(start);
    if (index < 0) return '(unmapped)';
    let best = null;
    let bestLength = 0;
    for (; index < spans.length && spans[index].start < end; index++) {
      const span = spans[index];
      const length = Math.min(span.end, end) - Math.max(span.start, start);
      if (length > bestLength) {
        bestLength = length;
        best = span;
      }
    }
    return best ? sourceOf(best) : '(unmapped)';
  };
  return { locate, locateLine };
}

/**
 * Aggregate a V8 sampling profile into self-time per source module.
 * Frames in the entry chunk map through its sourcemap; frames elsewhere bucket
 * by their script URL; engine frames (GC, parser, no URL) bucket as (engine).
 * Returns { rows: [{ bucket, ms }], totalMs } sorted by self-time.
 */
export function aggregateProfile(profile, { code, map, entryUrlSuffix, resolveChunk = null }) {
  // One locator per script: the entry chunk from `code`/`map`, every other chunk through
  // `resolveChunk(url)` -> { code, map } | null. Without it, a frame in a non-entry chunk
  // buckets by file name, and a shared chunk the entry imports statically hides the
  // modules it holds (checklist A4).
  const locators = new Map();
  const locatorFor = (url) => {
    if (url.endsWith(entryUrlSuffix)) {
      if (!locators.has('')) locators.set('', sourceLocator(code, map));
      return locators.get('');
    }
    if (!resolveChunk) return null;
    if (!locators.has(url)) {
      const chunk = resolveChunk(url);
      locators.set(url, chunk ? sourceLocator(chunk.code, chunk.map) : null);
    }
    return locators.get(url);
  };
  const nodesById = new Map(profile.nodes.map((node) => [node.id, node]));

  // Exact self-time per node from the sample stream.
  const selfMicros = new Map();
  const samples = profile.samples ?? [];
  const deltas = profile.timeDeltas ?? [];
  for (let i = 0; i < samples.length; i++) {
    const delta = deltas[i] ?? 0;
    if (delta <= 0) continue;
    selfMicros.set(samples[i], (selfMicros.get(samples[i]) ?? 0) + delta);
  }

  const buckets = new Map();
  const add = (bucket, micros) => buckets.set(bucket, (buckets.get(bucket) ?? 0) + micros);
  for (const [nodeId, micros] of selfMicros) {
    const node = nodesById.get(nodeId);
    if (!node) continue;
    const frame = node.callFrame ?? {};
    const fn = frame.functionName ?? '';
    if (fn === '(idle)' || fn === '(program)') continue; // not attributable work
    const locator = frame.url ? locatorFor(frame.url) : null;
    if (!frame.url) {
      add(fn === '(garbage collector)' ? '(engine: gc)' : '(engine)', micros);
    } else if (locator) {
      // Line ticks split a frame's self-time across the sources it spans. The chunk's
      // top-level frame is the one that matters: module evaluation of every module in
      // the chunk runs in it, and its callFrame position names none of them.
      const { locate, locateLine } = locator;
      const ticks = (node.positionTicks ?? []).filter((t) => t.ticks > 0);
      const total = ticks.reduce((sum, t) => sum + t.ticks, 0);
      if (total > 0) {
        for (const t of ticks) add(locateLine(t.line) ?? '(unmapped)', (micros * t.ticks) / total);
      } else {
        add(locate(frame.lineNumber ?? 0, frame.columnNumber ?? 0) ?? '(unmapped)', micros);
      }
    } else {
      add(frame.url.split('/').slice(-1)[0] || frame.url, micros);
    }
  }

  const rows = [...buckets.entries()]
    .map(([bucket, micros]) => ({ bucket, ms: Math.round(micros / 100) / 10 }))
    .filter((row) => row.ms >= 0.5)
    .sort((a, b) => b.ms - a.ms);
  const totalMs = Math.round([...buckets.values()].reduce((a, b) => a + b, 0) / 100) / 10;
  return { rows, totalMs };
}
