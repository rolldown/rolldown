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
  const spans = generatedSpans(code, map); // sorted [{start, end, srcIdx}]
  return (line, column) => {
    const lineStart = lineStarts[line];
    if (lineStart === undefined) return null;
    const offset = lineStart + column;
    let lo = 0;
    let hi = spans.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      const span = spans[mid];
      if (offset < span.start) hi = mid - 1;
      else if (offset >= span.end) lo = mid + 1;
      else
        return span.srcIdx >= 0
          ? String(map.sources[span.srcIdx] ?? `#${span.srcIdx}`).replaceAll('\\', '/')
          : '(unmapped)';
    }
    return '(unmapped)';
  };
}

/**
 * Aggregate a V8 sampling profile into self-time per source module.
 * Frames in the entry chunk map through its sourcemap; frames elsewhere bucket
 * by their script URL; engine frames (GC, parser, no URL) bucket as (engine).
 * Returns { rows: [{ bucket, ms }], totalMs } sorted by self-time.
 */
export function aggregateProfile(profile, { code, map, entryUrlSuffix }) {
  const locate = sourceLocator(code, map);
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
  for (const [nodeId, micros] of selfMicros) {
    const node = nodesById.get(nodeId);
    if (!node) continue;
    const frame = node.callFrame ?? {};
    const fn = frame.functionName ?? '';
    let bucket;
    if (fn === '(idle)' || fn === '(program)') continue; // not attributable work
    if (!frame.url) {
      bucket = fn === '(garbage collector)' ? '(engine: gc)' : '(engine)';
    } else if (frame.url.endsWith(entryUrlSuffix)) {
      bucket = locate(frame.lineNumber ?? 0, frame.columnNumber ?? 0) ?? '(unmapped)';
    } else {
      bucket = frame.url.split('/').slice(-1)[0] || frame.url;
    }
    buckets.set(bucket, (buckets.get(bucket) ?? 0) + micros);
  }

  const rows = [...buckets.entries()]
    .map(([bucket, micros]) => ({ bucket, ms: Math.round(micros / 100) / 10 }))
    .filter((row) => row.ms >= 0.5)
    .sort((a, b) => b.ms - a.ms);
  const totalMs = Math.round([...buckets.values()].reduce((a, b) => a + b, 0) / 100) / 10;
  return { rows, totalMs };
}
