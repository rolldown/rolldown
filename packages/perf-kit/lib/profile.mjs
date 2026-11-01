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
export function sourceLocator(code, map) {
  const lineStarts = [0];
  for (let i = 0; i < code.length; i++) {
    if (code.charCodeAt(i) === 10) lineStarts.push(i + 1);
  }
  const spans = generatedSpans(code, map); // sorted [{start, end, srcIdx}]
  const sourceName = (idx) => String(map.sources[idx] ?? `#${idx}`).replaceAll('\\', '/');
  const locate = (line, column) => {
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
      else return span.srcIdx >= 0 ? sourceName(span.srcIdx) : '(unmapped)';
    }
    return '(unmapped)';
  };
  /** The one source that a generated line holds, or null when the line mixes sources. */
  const lineSource = (line) => {
    const start = lineStarts[line];
    if (start === undefined) return null;
    const end = lineStarts[line + 1] ?? code.length;
    let lo = 0;
    let hi = spans.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (spans[mid].end <= start) lo = mid + 1;
      else hi = mid;
    }
    let found = null;
    for (let i = lo; i < spans.length && spans[i].start < end; i++) {
      const idx = spans[i].srcIdx;
      if (idx < 0) continue;
      if (found !== null && found !== idx) return null;
      found = idx;
    }
    return found === null ? null : sourceName(found);
  };
  return { locate, lineSource };
}

/**
 * Aggregate sampled self-time across all chunks, including dynamic imports.
 * Missing maps and unmapped frames remain explicit; a filename is not evidence
 * of engine work. readChunk(file) reads a local emitted file and its sourcemap.
 * Profiling stops when FCP is observed, so the polling tail can include some
 * post-paint work. Use these rows as leads, not exact paint-blocking costs.
 * `isLocal(url)` says which script URLs are local chunks (default: same origin).
 * `baseUrl` resolves relative script URLs (Miniflare reports bare module names).
 */
export function aggregateProfile(
  profile,
  { origin = profile.origin, isLocal = (url) => url.origin === origin, readChunk, baseUrl },
) {
  const nodesById = new Map(profile.nodes.map((node) => [node.id, node]));
  const parents = new Map();
  for (const node of profile.nodes)
    for (const child of node.children ?? []) parents.set(child, node);
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
          result = sourceLocator(chunk.code, chunk.map);
        }
      } catch {
        result = { reason: 'invalid-sourcemap' };
      }
      locators.set(file, result);
    }
    return locators.get(file);
  };

  const buckets = new Map();
  const add = (kind, bucket, file, micros) => {
    const key = JSON.stringify([kind, bucket]);
    const row = buckets.get(key) ?? { bucket, kind, micros: 0, chunks: new Set() };
    row.micros += micros;
    if (file) row.chunks.add(file);
    buckets.set(key, row);
  };
  for (const [nodeId, micros] of selfMicros) {
    let node = nodesById.get(nodeId);
    const fn = node?.callFrame?.functionName ?? '';
    if (fn === '(idle)' || fn === '(program)') continue;
    if (!node?.callFrame?.url && fn === '(garbage collector)') {
      add('engine', '(engine: gc)', null, micros);
      continue;
    }
    // A frame without a URL (a builtin or a host call: performance.now, JSON.parse,
    // a DOM method) does the work of the script that called it.
    while (node && !node.callFrame?.url) node = parents.get(node.id);
    if (!node) {
      add('engine', '(engine)', null, micros);
      continue;
    }
    const frame = node.callFrame;
    let url;
    try {
      url = new URL(frame.url, baseUrl);
    } catch {
      /* Unresolved eval/sourceURL frames stay unattributed. */
    }
    if (!url) {
      add('unmapped', frame.url, null, micros);
      continue;
    }
    if (!isLocal(url)) {
      add(
        'external',
        url.origin === 'null' ? frame.url : `${url.origin}${url.pathname}`,
        null,
        micros,
      );
      continue;
    }
    let file = null;
    try {
      const decoded = decodeURIComponent(url.pathname).replace(/^\/+/, '');
      // Never resolve encoded traversal or separators.
      if (
        !decoded.includes('\\') &&
        !decoded.split('/').some((part) => part === '..' || part === '.') &&
        /\.[cm]?js$/.test(decoded)
      )
        file = decoded;
    } catch {
      /* Invalid URL escapes stay unattributed. */
    }
    if (!file) {
      add('inline', url.pathname, null, micros);
      continue;
    }
    const locator = locateChunk(file);
    if (locator.reason) {
      add(locator.reason, file, file, micros);
    } else if (
      frame.functionName === '' &&
      ((frame.lineNumber === 0 && frame.columnNumber === 0) ||
        (frame.lineNumber === -1 && frame.columnNumber === -1))
    ) {
      // Top-level code (0:0 in Chrome, -1:-1 in workerd). A bundled chunk runs the
      // top-level code of many modules in one frame, so its start position names only
      // the first module. Split the time by the sampled lines; a line that holds more
      // than one source stays unattributed.
      const ticks = (node.positionTicks ?? []).filter((t) => t.ticks > 0);
      const total = ticks.reduce((sum, t) => sum + t.ticks, 0);
      if (!total) add('top-level', file, file, micros);
      for (const t of total ? ticks : []) {
        const source = locator.lineSource(t.line - 1);
        add(source ? 'source' : 'top-level', source ?? file, file, (micros * t.ticks) / total);
      }
    } else {
      const source = locator.locate(frame.lineNumber, frame.columnNumber);
      if (source && source !== '(unmapped)') add('source', source, file, micros);
      else add('unmapped', file, file, micros);
    }
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
  // A row under the candidate threshold cannot hide a source module that reaches it:
  // unattributed rows are kept per chunk file, so a module in one has at most its time.
  const unattributed = report.rows.filter(
    (row) => row.kind !== 'source' && row.kind !== 'engine' && row.ms >= 15,
  );
  return {
    state: candidates.length ? 'open' : unattributed.length ? 'unknown' : 'clear',
    candidates,
    unattributed,
  };
}
