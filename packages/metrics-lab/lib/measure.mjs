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
  const M = (window.__perfMetrics = { fcp: null, lcp: null, cls: 0 });
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
  } catch {}
})();`;

const COLLECT_JS = `(() => {
  const nav = performance.getEntriesByType('navigation')[0];
  const res = performance.getEntriesByType('resource');
  const M = window.__perfMetrics || {};
  return {
    fcp: M.fcp,
    lcp: M.lcp,
    cls: M.cls,
    ttfb: nav ? nav.responseStart : null,
    dcl: nav ? nav.domContentLoadedEventEnd : null,
    load: nav ? nav.loadEventEnd : null,
    bytes: (nav ? nav.encodedBodySize : 0) + res.reduce((a, r) => a + (r.encodedBodySize || 0), 0),
    jsRequests: res.filter((r) => r.name.endsWith('.js')).length,
    ready: Object.assign({}, window.__ready || {}),
    heroTitle: (document.getElementById('hero-title') || {}).textContent || '',
    heroSubtitle: (document.getElementById('hero-subtitle') || {}).textContent || '',
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

/** Fold N samples into the flat runtime metric-id map plus the correctness guard. */
export function summarize(samples, expectedFeatures = []) {
  const nums = (key) => samples.map((s) => s[key]).filter((v) => typeof v === 'number');
  const clsValues = nums('cls');
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
    },
    guard: {
      allFeaturesReady: samples.every(
        (s) => expectedFeatures.every((f) => s.ready && s.ready[f] === true),
      ),
      heroRendered: samples.every(
        (s) => (s.heroTitle || '').length > 0 && (s.heroSubtitle || '').length > 0,
      ),
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
