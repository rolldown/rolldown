// Timed loading runs: throttled navigations collecting web vitals via buffered
// PerformanceObservers injected before any page script runs.

import { openPage, sleep } from './cdp.mjs';

// Roughly Lighthouse's mobile lab shape: fast-3G-ish network, 4x CPU slowdown.
// Absolute numbers are lab-only; the signal is the delta between builds.
export const DEFAULT_THROTTLE = {
  latencyMs: 150,
  downloadBps: 200_000, // 1.6 Mbps
  uploadBps: 94_000,
  cpuRate: 4,
};

export const OBSERVER_JS = `(() => {
  const M = (window.__perfMetrics = { fcp: null, lcp: null, cls: 0, longtasks: [] });
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) M.lcp = e.startTime;
    }).observe({ type: 'largest-contentful-paint', buffered: true });
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) if (e.name === 'first-contentful-paint') M.fcp = e.startTime;
    }).observe({ type: 'paint', buffered: true });
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) if (!e.hadRecentInput) M.cls += e.value;
    }).observe({ type: 'layout-shift', buffered: true });
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) M.longtasks.push({ start: e.startTime, duration: e.duration });
    }).observe({ type: 'longtask', buffered: true });
  } catch {}
})();`;

const COLLECT_JS = `(() => {
  const nav = performance.getEntriesByType('navigation')[0];
  const res = performance.getEntriesByType('resource');
  const M = window.__perfMetrics || {};
  // What a resource IS (font/image/script/...), not just who initiated it -
  // fonts arrive with initiatorType 'css' or 'link', which hides them.
  const classify = (r) => {
    if (r.initiatorType === 'fetch') return 'fetch';
    if (r.initiatorType === 'xmlhttprequest') return 'xhr';
    const clean = r.name.split('?')[0].split('#')[0].toLowerCase();
    const ext = clean.slice(clean.lastIndexOf('.') + 1);
    if (['woff2', 'woff', 'ttf', 'otf', 'eot'].includes(ext)) return 'font';
    if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'avif', 'svg', 'ico', 'bmp'].includes(ext)) return 'image';
    if (['js', 'mjs', 'cjs'].includes(ext)) return 'script';
    if (ext === 'css') return 'css';
    if (ext === 'wasm' || ext === 'json') return ext;
    return r.initiatorType || 'other';
  };
  const all = res.map((r) => ({
    name: r.name.replace(location.origin, ''),
    type: classify(r),
    start: Math.round(r.startTime),
    end: Math.round(r.responseEnd),
    bytes: r.encodedBodySize || 0,
  }));
  // Uncapped per-type totals (+ the pre-first-paint share): real apps load
  // hundreds of resources, so any capped detail list under-reports by design.
  const typeAgg = {};
  for (const r of all) {
    const a = typeAgg[r.type] || (typeAgg[r.type] = { count: 0, bytes: 0, preFcpCount: 0, preFcpBytes: 0 });
    a.count += 1;
    a.bytes += r.bytes;
    // end 0 = still in flight or failed, not "finished before paint". Bytes stay
    // 0 for cross-origin responses without Timing-Allow-Origin - count anyway.
    if (typeof M.fcp === 'number' && r.end > 0 && r.end <= M.fcp) {
      a.preFcpCount += 1;
      a.preFcpBytes += r.bytes;
    }
  }
  // Render-blocking resources gate FCP by spec: nothing paints until the last
  // blocking stylesheet arrives and parses. Aggregate uncapped (the detail list
  // is capped) - the JS-side signals can never see this class of gate.
  const blocking = res.filter((r) => r.renderBlockingStatus === 'blocking');
  const blockingGate = {
    count: blocking.length,
    bytes: blocking.reduce((a, r) => a + (r.encodedBodySize || 0), 0),
    lastEndMs: Math.round(blocking.reduce((a, r) => Math.max(a, r.responseEnd || 0), 0)),
    worst: blocking.slice().sort((a, b) => (b.responseEnd || 0) - (a.responseEnd || 0)).slice(0, 4)
      .map((r) => ({ name: r.name.replace(location.origin, ''), end: Math.round(r.responseEnd || 0), bytes: r.encodedBodySize || 0 })),
  };
  // Detail list: every fetch/xhr (render-gap suspects are often tiny), then the
  // largest of everything else - a big resource must never fall off the list.
  const fetchLike = all.filter((r) => r.type === 'fetch' || r.type === 'xhr').slice(0, 40);
  const rest = all.filter((r) => r.type !== 'fetch' && r.type !== 'xhr')
    .sort((a, b) => b.bytes - a.bytes)
    .slice(0, 60 - fetchLike.length);
  return {
    blockingGate,
    fcp: M.fcp,
    lcp: M.lcp,
    cls: M.cls,
    ttfb: nav ? nav.responseStart : null,
    dcl: nav ? nav.domContentLoadedEventEnd : null,
    load: nav ? nav.loadEventEnd : null,
    bytes: (nav ? nav.encodedBodySize : 0) + res.reduce((a, r) => a + (r.encodedBodySize || 0), 0),
    jsRequests: res.filter((r) => r.name.endsWith('.js')).length,
    resources: fetchLike.concat(rest),
    resourceTypes: typeAgg,
    longtasks: (M.longtasks || []).map((t) => ({ start: Math.round(t.start), duration: Math.round(t.duration) })),
    ready: Object.assign({}, window.__ready || {}),
    heroTitle: (() => { const el = document.getElementById('hero-title'); return el ? el.textContent || '' : null; })(),
    heroSubtitle: (() => { const el = document.getElementById('hero-subtitle'); return el ? el.textContent || '' : null; })(),
  };
})()`;

function readyExpr(expectedFeatures) {
  return `(() => {
    const nav = performance.getEntriesByType('navigation')[0];
    const M = window.__perfMetrics || {};
    const r = window.__ready || {};
    return {
      load: nav ? nav.loadEventEnd : 0,
      lcp: M.lcp,
      ready: ${JSON.stringify(expectedFeatures)}.every((f) => r[f] === true),
    };
  })()`;
}

/** One navigation; resolves with the collected sample (missing vitals stay null). */
export async function timedRun(cdp, { url, throttle, expectedFeatures = [], settleMs = 1500, timeoutMs = 60_000 }) {
  const page = await openPage(cdp, { throttle, injectScript: OBSERVER_JS });
  try {
    await page.navigate(url);
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const state = await page.evaluate(readyExpr(expectedFeatures));
      if (state.load > 0 && state.lcp != null && state.ready) break;
      if (Date.now() > deadline) break; // collect what we have; the guard will flag it
      await sleep(100);
    }
    await sleep(settleMs);
    return await page.evaluate(COLLECT_JS);
  } finally {
    await page.close().catch(() => {});
  }
}

export function median(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

export function quantile(values, q) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.min(sorted.length - 1, Math.ceil(q * sorted.length) - 1);
  return sorted[Math.max(0, idx)];
}

const round1 = (v) => (v == null ? null : Math.round(v * 10) / 10);
const round3 = (v) => (v == null ? null : Math.round(v * 1000) / 1000);

/** Fetch/XHR requests that completed before first paint — the render-gating suspects. */
function gatingFetches(samples) {
  const seen = new Map();
  for (const sample of samples) {
    if (typeof sample.fcp !== 'number') continue;
    for (const resource of sample.resources ?? []) {
      if (resource.type !== 'fetch' && resource.type !== 'xhr') continue;
      if (resource.end <= 0 || resource.end > sample.fcp) continue;
      seen.set(resource.name, `${resource.name} (${resource.type}, finished ${Math.round(sample.fcp - resource.end)}ms before first paint)`);
    }
  }
  return [...seen.values()];
}

/** Per-type resource totals folded across samples (median per field, missing = 0). */
function resourceWeight(samples) {
  const types = new Set();
  for (const sample of samples) {
    for (const type of Object.keys(sample.resourceTypes ?? {})) types.add(type);
  }
  // preFcp fields are only meaningful in runs where FCP was observed.
  const painted = samples.filter((s) => typeof s.fcp === 'number');
  const kb = (bytes) => Math.round(((bytes ?? 0) / 1024) * 10) / 10;
  const rows = [];
  for (const type of types) {
    const field = (pool, name) => median(pool.map((s) => s.resourceTypes?.[type]?.[name] ?? 0)) ?? 0;
    const row = {
      type,
      count: Math.round(field(samples, 'count')),
      kb: kb(field(samples, 'bytes')),
      preFcpCount: painted.length ? Math.round(field(painted, 'preFcpCount')) : 0,
      preFcpKb: painted.length ? kb(field(painted, 'preFcpBytes')) : 0,
    };
    if (row.count > 0) rows.push(row);
  }
  return rows.sort((a, b) => b.preFcpKb - a.preFcpKb || b.kb - a.kb);
}

/** "3 font 240KB" — or "3 font" when sizes are hidden (cross-origin without Timing-Allow-Origin). */
export function weightLabel(row) {
  const kb = Math.round(row.preFcpKb);
  return `${row.preFcpCount} ${row.type}${kb >= 1 ? ` ${kb}KB` : ''}`;
}

/**
 * Non-fetch resource types heavy enough before first paint to plausibly gate
 * rendering — the fonts/images a fetch-only render-gap analysis is blind to.
 */
export function heavyPrepaintTypes(weightRows) {
  return (weightRows ?? []).filter((row) => (
    (row.type === 'font' && (row.preFcpKb >= 50 || row.preFcpCount >= 5))
    || (row.type === 'image' && row.preFcpKb >= 100)
  ));
}

/**
 * FCP cannot precede the last render-blocking stylesheet (spec: nothing paints
 * until blocking CSS arrives and parses). When that stylesheet occupies a large
 * share of the FCP timeline, CSS is the paint gate — a class every JS-side
 * signal is blind to. (jellyfin A/B 2026-07-11: unblocking CSS scored −50.1%
 * where the JS-lead path got −36.5%; the harness had no CSS signal at all, and
 * a "finished within 300ms of FCP" rule ALSO missed it — the CSS finished 1s
 * before FCP yet held 77% of its timeline. Judge by share, not adjacency.)
 */
export function renderBlockingGate(samples) {
  const painted = samples.filter((s) => typeof s.fcp === 'number' && s.blockingGate);
  if (!painted.length) return null;
  const count = Math.max(...painted.map((s) => s.blockingGate.count ?? 0));
  if (!count) return null;
  const fcpMs = median(painted.map((s) => s.fcp));
  const lastEndMs = median(painted.map((s) => s.blockingGate.lastEndMs ?? 0));
  const kb = Math.round((median(painted.map((s) => s.blockingGate.bytes ?? 0)) ?? 0) / 1024);
  const mid = painted.slice().sort((a, b) => a.fcp - b.fcp)[Math.floor(painted.length / 2)];
  const shareOfFcp = fcpMs > 0 ? (lastEndMs ?? 0) / fcpMs : 0;
  return {
    count,
    kb,
    lastEndMs: Math.round(lastEndMs ?? 0),
    fcpMs: Math.round(fcpMs ?? 0),
    shareOfFcp: Math.round(shareOfFcp * 100) / 100,
    gating: shareOfFcp >= 0.4 && kb >= 8,
    worst: mid.blockingGate.worst ?? [],
  };
}

/** Fold N samples into the flat runtime metric-id map plus the correctness guard. */
export function summarize(samples, expectedFeatures = []) {
  const nums = (key) => samples.map((s) => s[key]).filter((v) => typeof v === 'number');
  const clsValues = nums('cls');
  const renderGaps = samples
    .filter((s) => typeof s.lcp === 'number' && typeof s.load === 'number')
    .map((s) => s.lcp - s.load);
  const prepaintLongtask = samples
    .filter((s) => typeof s.fcp === 'number')
    .map((s) => (s.longtasks ?? [])
      .filter((t) => t.start < s.fcp)
      .reduce((sum, t) => sum + t.duration, 0));
  return {
    runs: samples.length,
    metrics: {
      'runtime.lcp_ms': round1(median(nums('lcp'))),
      'runtime.lcp_p75_ms': round1(quantile(nums('lcp'), 0.75)),
      'runtime.fcp_ms': round1(median(nums('fcp'))),
      'runtime.ttfb_ms': round1(median(nums('ttfb'))),
      'runtime.load_ms': round1(median(nums('load'))),
      'runtime.cls': round3(clsValues.length ? Math.max(...clsValues) : null),
      'runtime.transfer_bytes': Math.round(median(nums('bytes')) ?? 0),
      'runtime.js_request_count': Math.round(median(nums('jsRequests')) ?? 0),
      // How long after `load` the largest paint landed: a big gap means rendering
      // is gated on post-load work (an awaited fetch, a lazily fetched chunk, CPU).
      'runtime.render_gap_ms': round1(median(renderGaps)),
      // Long-task time before first paint: boot CPU running ahead of render.
      'runtime.prepaint_longtask_ms': round1(median(prepaintLongtask)),
    },
    gatingFetches: gatingFetches(samples),
    resourceWeight: resourceWeight(samples),
    renderBlockingGate: renderBlockingGate(samples),
    guard: {
      allFeaturesReady: samples.every(
        (s) => expectedFeatures.every((f) => s.ready && s.ready[f] === true),
      ),
      // null when the page has no hero probe elements (measuring an arbitrary app
      // via --dist); the demo app must render both.
      heroRendered: samples.some((s) => s.heroTitle !== null || s.heroSubtitle !== null)
        ? samples.every(
          (s) => (s.heroTitle ?? '').length > 0 && (s.heroSubtitle === null || s.heroSubtitle.length > 0),
        )
        : null,
      lcpObservedInAllRuns: samples.every((s) => typeof s.lcp === 'number'),
    },
    samples,
  };
}

/** Per-metric prev/curr/delta/pct — the same shape as build metrics.json deltas. */
export function deltaSection(prevMetrics, currMetrics) {
  const out = {};
  for (const [id, curr] of Object.entries(currMetrics)) {
    const prev = prevMetrics?.[id];
    if (typeof prev !== 'number' || typeof curr !== 'number') continue;
    const delta = curr - prev;
    out[id] = {
      prev,
      curr,
      delta: Math.round(delta * 1000) / 1000,
      pct: prev === 0 ? null : Math.round((delta / prev) * 1000) / 10,
    };
  }
  return out;
}
