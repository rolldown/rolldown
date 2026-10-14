// Pure analysis for field runs (a deployed site, a real login, a real network):
// the request the LCP waited for, the script chain that started it, paired
// statistics across alternating variants, chunk-level coverage that needs no
// sourcemap, and URL redaction. No browser access here, so all of it is testable.

import { coverageBounds, executedIntervals } from '../coverage.mjs';
import { median, quantile } from '../measure.mjs';

// --- redaction -------------------------------------------------------------------
// Field output comes from a real account. Identifiers in URLs (account, zone and
// user IDs, emails, long numbers) are replaced before anything is written.

const DEFAULT_REDACTIONS = [
  [/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi, '<uuid>'],
  [/[0-9a-f]{16,}/gi, '<hex>'],
  [/[^\s/?&=@:]+@[^\s/?&=]+\.[a-z]{2,}/gi, '<email>'],
  [/(?<![\w.])\d{6,}(?![\w])/g, '<n>'],
];

export function makeRedactor(extraPatterns = []) {
  const rules = [
    ...DEFAULT_REDACTIONS,
    ...extraPatterns.map((source) => [new RegExp(source, 'g'), '<redacted>']),
  ];
  return (value) => {
    if (typeof value !== 'string') return value;
    let out = value;
    for (const [re, replacement] of rules) out = out.replace(re, replacement);
    return out;
  };
}

// --- critical path ---------------------------------------------------------------

const isDataRequest = (r) => r.type === 'Fetch' || r.type === 'XHR';

/**
 * The data request the LCP most likely waited for: the fetch/XHR that finished
 * last before LCP (within `windowMs`), or the last one matching `pattern` when the
 * config pins it. The automatic choice is a heuristic - `candidates` lists the
 * runners-up so a wrong pick is visible, and `gate` in the config fixes it.
 */
export function findGate(requests, lcpMs, { pattern = null, windowMs = 1500 } = {}) {
  if (typeof lcpMs !== 'number') return { gate: null, reason: 'no LCP', candidates: [] };
  let pool = requests.filter(
    (r) => isDataRequest(r) && !r.failed && typeof r.endMs === 'number' && r.endMs <= lcpMs,
  );
  if (pattern) pool = pool.filter((r) => r.url.includes(pattern));
  else pool = pool.filter((r) => lcpMs - r.endMs <= windowMs);
  pool.sort((a, b) => b.endMs - a.endMs);
  const candidates = pool.slice(0, 3).map((r) => ({ url: r.url, endMs: r.endMs }));
  if (!pool.length) {
    return {
      gate: null,
      reason: pattern
        ? `no request matching "${pattern}" finished before LCP`
        : `no fetch/XHR finished within ${windowMs}ms before LCP`,
      candidates,
    };
  }
  return { gate: pool[0], reason: pattern ? 'pinned by config' : 'auto', candidates };
}

/**
 * The script a request had to wait for. A request cannot start before every
 * script on its initiator stack has loaded, so of all the scripts on the stack
 * (sync frames and async parents) the one whose download ended last is the
 * bottleneck. A fetch wrapper on top of the stack is then skipped automatically
 * when the real caller loaded later.
 */
export function criticalInitiatorUrl(req, requests) {
  const urls = [
    ...new Set([...(req.initiator?.frames ?? []).map((f) => f.url), req.initiator?.url]),
  ].filter(Boolean);
  let best = null;
  for (const url of urls) {
    const loaded = requests.find((r) => r.url === url && !r.failed);
    if (loaded && (!best || (loaded.endMs ?? -1) > (best.endMs ?? -1))) best = loaded;
  }
  return best?.url ?? urls[0] ?? null;
}

/**
 * Walk initiators back from a request: the script it waited for, the script that
 * loaded that script, and so on to the document. Each step keeps its fetch
 * window, which is what turns "the request started late" into the exact
 * waterfall of chunk loads in front of it.
 */
export function initiatorChain(requests, startUrl, maxDepth = 12) {
  const chain = [];
  const seen = new Set();
  let url = startUrl;
  while (url && !seen.has(url) && chain.length < maxDepth) {
    seen.add(url);
    const req = requests.find((r) => r.url === url && !r.failed);
    if (!req) {
      chain.push({
        url,
        type: 'unknown',
        note: 'no request recorded for this URL (inline or cached)',
      });
      break;
    }
    chain.push({
      url: req.url,
      type: req.type,
      startMs: req.startMs,
      endMs: req.endMs,
      initiatorType: req.initiator?.type ?? null,
    });
    if (req.type === 'Document') break;
    url = criticalInitiatorUrl(req, requests);
  }
  return chain;
}

/**
 * Split one run's LCP at its gate: the client time before the gate request
 * started (also measured from the end of the document, which removes a slower
 * HTML response of one variant's server), the time to first byte, the
 * download, and the client time after it. Only the client parts can move with
 * a front-end change.
 */
export function criticalPath(requests, lcpMs, options = {}) {
  const found = findGate(requests, lcpMs, options);
  if (!found.gate) return { ...found, breakdown: null, chain: [] };
  const g = found.gate;
  const sendStart = g.sendStartMs ?? g.startMs;
  const headersEnd = g.headersEndMs ?? null;
  const doc = requests.find((r) => r.type === 'Document' && !r.failed);
  return {
    gate: { url: g.url, method: g.method, startMs: g.startMs, endMs: g.endMs },
    reason: found.reason,
    candidates: found.candidates,
    breakdown: {
      documentEndMs: round(doc?.endMs ?? null),
      beforeGateMs: round(g.startMs),
      gateAfterDocumentMs: doc?.endMs == null ? null : round(g.startMs - doc.endMs),
      ttfbMs: headersEnd == null ? null : round(headersEnd - sendStart),
      downloadMs: headersEnd == null ? null : round(g.endMs - headersEnd),
      afterGateMs: round(lcpMs - g.endMs),
    },
    chain: initiatorChain(requests, criticalInitiatorUrl(g, requests)),
  };
}

// --- statistics ------------------------------------------------------------------

function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Paired comparison of alternating runs: pair i holds one run of each variant,
 * measured back to back, so slow minutes of the network hit both sides. Reports
 * the median paired delta (candidate - base; negative = candidate faster), a
 * seeded bootstrap 90% interval of that median, and the win count.
 */
export function pairedDelta(pairs, { resamples = 2000, seed = 1 } = {}) {
  const deltas = pairs
    .filter(([a, b]) => typeof a === 'number' && typeof b === 'number')
    .map(([a, b]) => b - a);
  if (!deltas.length) return null;
  const rand = mulberry32(seed);
  const medians = [];
  for (let i = 0; i < resamples; i++) {
    const draw = deltas.map(() => deltas[Math.floor(rand() * deltas.length)]);
    medians.push(median(draw));
  }
  return {
    pairs: deltas.length,
    medianDeltaMs: round(median(deltas)),
    ci90Ms: [round(quantile(medians, 0.05)), round(quantile(medians, 0.95))],
    candidateFaster: deltas.filter((d) => d < 0).length,
  };
}

/** Values above `factor` x the median, for n >= 3. A slow server minute, not the build. */
export function outlierIndexes(values, factor = 3) {
  const nums = values.filter((v) => typeof v === 'number');
  if (nums.length < 3) return [];
  const mid = median(nums);
  return values.flatMap((v, i) => (typeof v === 'number' && v > factor * mid ? [i] : []));
}

// --- chunk-level coverage ----------------------------------------------------------

function unionLength(a, b) {
  const all = [...a, ...b].sort((x, y) => x.start - y.start);
  let total = 0;
  let current = null;
  for (const interval of all) {
    if (current && interval.start <= current.end) current.end = Math.max(current.end, interval.end);
    else {
      if (current) total += current.end - current.start;
      current = { ...interval };
    }
  }
  if (current) total += current.end - current.start;
  return total;
}

/**
 * Executed bytes of one served script at the cut and by settle, from V8 ranges
 * and the response body alone. Works for every chunk, with or without a matching
 * local build - module attribution is what needs the build.
 */
export function chunkCoverage({ bytes, atCut, atSettle }) {
  const cut = atCut ? executedIntervals(coverageBounds(atCut)) : [];
  const settle = atSettle ? executedIntervals(coverageBounds(atSettle)) : [];
  const cutBytes = Math.min(bytes, unionLength(cut, []));
  const settleBytes = Math.min(bytes, unionLength(cut, settle));
  return { bytes, cutBytes, settleBytes, cutRatio: bytes ? cutBytes / bytes : 0 };
}

export const round = (v) => (typeof v === 'number' ? Math.round(v) : null);
