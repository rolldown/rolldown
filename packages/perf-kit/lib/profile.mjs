// Boot CPU profile: sample from navigation until FCP is observed, then
// attribute self-time through every sampled same-origin chunk's sourcemap.
// Answers the question coverage can't: "what RAN before paint, and for how long?"
// — which is exactly the evidence needed to judge whether eager boot work
// (telemetry, warm-up caches, data-module evaluation) belongs on the critical path.

import { openPage, sleep } from './cdp.mjs';
import { generatedSpans } from './coverage.mjs';
import { OBSERVER_JS } from './measure.mjs';

/**
 * One profiled navigation. Arms the sampling profiler on the same-origin blank
 * page, navigates, and stops when FCP is observed. The polling tail can include
 * post-paint work; these samples are leads rather than exact paint costs.
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
    return { ...profile, origin };
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
    if (!Number.isInteger(line) || !Number.isInteger(column) || line < 0 || column < 0) return null;
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
 * Aggregate sampled self-time across all chunks, including dynamic imports.
 * Missing maps and unmapped frames remain explicit; a filename is not evidence
 * of engine work. readChunk(file) reads a local emitted file and its sourcemap.
 * Profiling stops when FCP is observed, so the polling tail can include some
 * post-paint work. Use these rows as leads, not exact paint-blocking costs.
 */
export function aggregateProfile(profile, { origin = profile.origin, readChunk }) {
  const nodesById = new Map(profile.nodes.map((node) => [node.id, node]));
  const selfMicros = new Map();
  const samples = profile.samples ?? [];
  const deltas = profile.timeDeltas ?? [];
  for (let i = 0; i < samples.length; i++) {
    const delta = deltas[i] ?? 0;
    if (!Number.isFinite(delta) || delta <= 0) continue;
    selfMicros.set(samples[i], (selfMicros.get(samples[i]) ?? 0) + delta);
  }

  const locators = new Map();
  const locateChunk = (file) => {
    if (!locators.has(file)) {
      let result;
      try {
        const chunk = readChunk(file);
        if (!chunk) {
          result = { reason: 'no-sourcemap' };
        } else if (
          typeof chunk.code !== 'string' ||
          !Array.isArray(chunk.map?.sources) ||
          typeof chunk.map?.mappings !== 'string'
        ) {
          result = { reason: 'invalid-sourcemap' };
        } else {
          result = { locate: sourceLocator(chunk.code, chunk.map) };
        }
      } catch {
        result = { reason: 'invalid-sourcemap' };
      }
      locators.set(file, result);
    }
    return locators.get(file);
  };

  const buckets = new Map();
  for (const [nodeId, micros] of selfMicros) {
    const frame = nodesById.get(nodeId)?.callFrame ?? {};
    const fn = frame.functionName ?? '';
    if (fn === '(idle)' || fn === '(program)') continue;
    let kind = 'unmapped';
    let bucket = '(anonymous)';
    let file = null;
    if (!frame.url) {
      if (fn === '(garbage collector)') {
        kind = 'engine';
        bucket = '(engine: gc)';
      }
    } else {
      let url;
      try {
        url = new URL(frame.url);
      } catch {
        /* Unresolved eval/sourceURL frames stay unattributed. */
      }
      if (url && url.origin !== origin) {
        kind = 'external';
        bucket = `${url.origin}${url.pathname}`;
      } else if (url) {
        try {
          const decoded = decodeURIComponent(url.pathname).slice(1);
          // Never resolve encoded traversal, separators, or absolute paths on disk.
          if (
            !decoded.startsWith('/') &&
            !decoded.includes('\\') &&
            !decoded.split('/').some((part) => part === '..' || part === '.') &&
            /\.[cm]?js$/.test(decoded)
          )
            file = decoded;
        } catch {
          /* Invalid URL escapes stay unattributed. */
        }
        bucket = url.pathname;
        if (file) {
          const locator = locateChunk(file);
          const source = locator.locate?.(frame.lineNumber, frame.columnNumber);
          kind = locator.reason ?? (source && source !== '(unmapped)' ? 'source' : 'unmapped');
          bucket = kind === 'source' ? source : file;
        } else {
          kind = 'inline';
        }
      } else {
        bucket = frame.url;
      }
    }
    const key = JSON.stringify([kind, bucket]);
    const row = buckets.get(key) ?? { bucket, kind, micros: 0, chunks: new Set() };
    row.micros += micros;
    if (file) row.chunks.add(file);
    buckets.set(key, row);
  }

  const rows = [...buckets.values()]
    .map(({ bucket, kind, micros, chunks }) => ({
      bucket,
      kind,
      ms: Math.round(micros / 100) / 10,
      chunks: [...chunks].sort((a, b) => a.localeCompare(b)),
    }))
    .sort((a, b) => b.ms - a.ms || a.bucket.localeCompare(b.bucket));
  const sumMs = (predicate) =>
    Math.round(
      [...buckets.values()].filter(predicate).reduce((sum, row) => sum + row.micros, 0) / 100,
    ) / 10;
  return {
    scope: 'all-chunks',
    rows,
    totalMs: sumMs(() => true),
    attribution: {
      sourceMs: sumMs((r) => r.kind === 'source'),
      engineMs: sumMs((r) => r.kind === 'engine'),
      unattributedMs: sumMs((r) => r.kind !== 'source' && r.kind !== 'engine'),
    },
  };
}

/** A sampled module is a source lead, never proof that its work is optional. */
export function profileEvidence(report) {
  if (report?.scope !== 'all-chunks') {
    return {
      state: 'unknown',
      reason: 'profile does not attribute all sampled chunks',
      candidates: [],
      unattributed: [],
    };
  }
  if (!report.rows?.length) {
    return {
      state: 'unknown',
      reason: 'profile has no sampled work',
      candidates: [],
      unattributed: [],
    };
  }
  const candidates = report.rows.filter((row) => row.kind === 'source' && row.ms >= 15);
  const unattributed = report.rows.filter((row) => row.kind !== 'source' && row.kind !== 'engine');
  return {
    state: candidates.length ? 'open' : unattributed.length ? 'unknown' : 'clear',
    candidates,
    unattributed,
  };
}
