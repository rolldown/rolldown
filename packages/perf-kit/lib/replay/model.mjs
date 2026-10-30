// Pure parts of record-and-replay: request keys, volatile query parameters, the
// archive built from recorded runs, the response index the replay server matches
// against, and the network model fitted from recorded downloads. No I/O here, so
// all of it is testable.
//
// Why a model and not per-URL timing: a changed build has other chunk names and
// sizes, so recorded asset timings do not apply to it. The replay serves the
// local dist through a fitted network (round trip, CDN time to first byte, ONE
// shared bandwidth) and serves recorded API responses after their recorded
// server time.

import crypto from 'node:crypto';

import { median, quantile } from '../measure.mjs';

export const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');

/** The archive format version. `replay` refuses any other: record again. */
export const ARCHIVE_SCHEMA = 1;

const round = (v) => (typeof v === 'number' ? Math.round(v) : null);

// --- keys ------------------------------------------------------------------------

const routeKeyOf = (method, u) => `${method} ${u.host}${u.pathname}`;

/**
 * method + host + path + sorted query, without the volatile parameters of that
 * route. Two loads of the same page give the same key for the same request.
 */
export function requestKey(method, url, volatile = {}) {
  const u = new URL(url);
  const drop = new Set(volatile[routeKeyOf(method, u)] ?? []);
  const params = [...u.searchParams]
    .filter(([name]) => !drop.has(name))
    .sort(([a, av], [b, bv]) => (a === b ? (av < bv ? -1 : av > bv ? 1 : 0) : a < b ? -1 : 1));
  const query = new URLSearchParams(params).toString();
  return `${routeKeyOf(method, u)}${query ? `?${query}` : ''}`;
}

const isHttp = (url) => /^https?:\/\//.test(url ?? '');

/**
 * Query parameters whose values change between loads of the same page (time
 * stamps, cache busters, random IDs). A parameter is volatile for a route when
 * the sorted list of its values differs between two runs. A parameter with
 * several values in ONE run but the same list in every run (page=1, page=2) is
 * stable. A route seen in one run only has no volatile parameters.
 */
export function findVolatileParams(runs) {
  const routes = new Map(); // routeKey -> Map(runIndex -> Map(param -> values[]))
  runs.forEach((requests, runIndex) => {
    for (const r of requests) {
      if (!isHttp(r.url)) continue;
      const u = new URL(r.url);
      const route = routeKeyOf(r.method, u);
      const byRun = routes.get(route) ?? new Map();
      routes.set(route, byRun);
      const params = byRun.get(runIndex) ?? new Map();
      byRun.set(runIndex, params);
      for (const [name, value] of u.searchParams) {
        params.set(name, [...(params.get(name) ?? []), value]);
      }
    }
  });
  const volatile = {};
  for (const [route, byRun] of routes) {
    if (byRun.size < 2) continue;
    const names = new Set([...byRun.values()].flatMap((p) => [...p.keys()]));
    const changing = [...names].filter((name) => {
      const lists = [...byRun.values()].map((p) =>
        JSON.stringify([...(p.get(name) ?? [])].sort((a, b) => a.localeCompare(b))),
      );
      return new Set(lists).size > 1;
    });
    if (changing.length) volatile[route] = changing.sort((a, b) => a.localeCompare(b));
  }
  return volatile;
}

// Request bodies often carry a time stamp or a random ID; the normalized hash
// still matches them.
const VOLATILE_TEXT = [
  [/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?/g, '<time>'],
  [/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi, '<uuid>'],
  [/(?<![\w.])\d{10,}(?:\.\d+)?(?![\w])/g, '<n>'],
];

export function normalizeBody(text) {
  let out = String(text);
  for (const [re, replacement] of VOLATILE_TEXT) out = out.replace(re, replacement);
  return out;
}

export const bodyHashes = (body) =>
  body == null || body === ''
    ? { bodyHash: null, bodyNormHash: null }
    : { bodyHash: sha256(body), bodyNormHash: sha256(normalizeBody(body)) };

// --- response headers ------------------------------------------------------------
// Only headers that change how the browser treats a response are kept. Never
// cookies, never auth, never server or tracing identifiers.

const KEEP_HEADERS = new Set([
  'content-type',
  'content-encoding',
  'content-language',
  'vary',
  'location',
  'link',
  'access-control-allow-origin',
  'access-control-allow-credentials',
  'access-control-allow-headers',
  'access-control-allow-methods',
  'access-control-expose-headers',
  'access-control-max-age',
  'timing-allow-origin',
  'cross-origin-opener-policy',
  'cross-origin-embedder-policy',
  'cross-origin-resource-policy',
  'x-content-type-options',
]);

export function safeHeaders(headers = {}) {
  const out = {};
  for (const [name, value] of Object.entries(headers)) {
    const key = name.toLowerCase();
    if (KEEP_HEADERS.has(key)) out[key] = String(value);
  }
  return out;
}

// --- network model ---------------------------------------------------------------

const STATIC_TYPES = new Set(['Script', 'Stylesheet', 'Font', 'Image', 'Media', 'Manifest']);

export const isStaticRequest = (r, origin) =>
  r.method === 'GET' &&
  STATIC_TYPES.has(r.type) &&
  typeof r.url === 'string' &&
  r.url.startsWith(`${origin}/`);

const done = (r) =>
  !r.failed &&
  typeof r.sendStartMs === 'number' &&
  typeof r.headersEndMs === 'number' &&
  typeof r.endMs === 'number';

/**
 * Busiest window of received bytes in one run. Each response's bytes are spread
 * evenly over its download (headers end -> end). This is a lower bound of the
 * access link: it reaches the link rate only when downloads saturated it.
 */
export function peakThroughputBps(requests, windowMs = 250) {
  const bins = new Map();
  for (const r of requests) {
    if (!done(r) || !(r.encodedBytes > 0)) continue;
    const start = r.headersEndMs;
    const end = Math.max(r.endMs, start + 0.001);
    const perMs = r.encodedBytes / (end - start);
    for (let b = Math.floor(start / windowMs); b * windowMs < end; b++) {
      const overlap = Math.min(end, (b + 1) * windowMs) - Math.max(start, b * windowMs);
      if (overlap > 0) bins.set(b, (bins.get(b) ?? 0) + overlap * perMs);
    }
  }
  const peak = Math.max(0, ...bins.values());
  return (peak / windowMs) * 1000;
}

/** 21 points (p0, p5, ..., p100): a distribution small enough to keep, rich enough to sample. */
export function quantilePoints(values) {
  if (!values.length) return null;
  return Array.from({ length: 21 }, (_, i) => Math.round(quantile(values, i / 20)));
}

/** Inverse CDF: u in [0, 1) -> a value, linear between the points. */
export function sampleQuantiles(points, u) {
  const pos = Math.min(Math.max(u, 0), 1) * (points.length - 1);
  const lo = Math.floor(pos);
  return lo >= points.length - 1
    ? points[points.length - 1]
    : points[lo] + (points[lo + 1] - points[lo]) * (pos - lo);
}

/** Seeded PRNG: a pair of runs (one per build) gets the same network luck. */
export function seededRandom(seed) {
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
 * Bytes that OTHER responses received while `r` waited for its first byte.
 * Each response's bytes are spread evenly over its download.
 */
function otherBytesDuringWait(r, requests) {
  let bytes = 0;
  for (const o of requests) {
    if (o === r || !done(o) || !(o.encodedBytes > 0)) continue;
    const end = Math.max(o.endMs, o.headersEndMs + 0.001);
    const overlap = Math.min(end, r.headersEndMs) - Math.max(o.headersEndMs, r.sendStartMs);
    if (overlap > 0) bytes += (o.encodedBytes * overlap) / (end - o.headersEndMs);
  }
  return bytes;
}

const IDLE_LINK_BYTES = 50_000;

/**
 * Network model from recorded runs (page-ms request rows, see recordFieldNetwork):
 *   staticTtfbMs / staticTtfbPoints - time to first byte of same-origin static
 *       files while the link was idle (other responses received under 50 kB
 *       during the wait): RTT + CDN time. The replay samples the distribution:
 *       a startup step waits for the SLOWEST of its parallel chunks, so a
 *       constant median makes every step too fast.
 *   wireBytes    - how far behind other bytes a response's headers queue on a
 *       busy link (send buffer + bytes in flight): the median extra TTFB of the
 *       loaded requests x bandwidth. On a slow link most of a long TTFB is this
 *       queue, caused by the page's own downloads, so the replay must queue the
 *       headers (see shaper.mjs) and not draw the long TTFBs a second time.
 *   rttMs        - 10th percentile of all static TTFBs: the closest thing to a bare round trip
 *   bandwidthBps - median over runs of the busiest 250 ms window. A lower bound
 *       of the link; the fastest single download over-reads (a body that arrives
 *       in one burst after its headers looks infinitely fast)
 * The replay applies bandwidthBps to ALL streams together: a per-request delay
 * would let parallel downloads all run at full speed.
 */
export function fitNetworkModel(runs, origin) {
  const ttfbs = [];
  const waits = [];
  const perRun = [];
  for (const requests of runs) {
    const statics = requests.filter((r) => isStaticRequest(r, origin) && done(r));
    for (const r of statics) {
      const ttfb = r.headersEndMs - r.sendStartMs;
      ttfbs.push(ttfb);
      waits.push({ ttfb, otherBytes: otherBytesDuringWait(r, requests) });
    }
    const single = Math.max(
      0,
      ...requests
        .filter((r) => done(r) && r.encodedBytes >= 100_000 && r.endMs > r.headersEndMs)
        .map((r) => (r.encodedBytes / (r.endMs - r.headersEndMs)) * 1000),
    );
    perRun.push({ window: peakThroughputBps(requests), single });
  }
  const bandwidth = median(perRun.map((x) => x.window).filter((v) => v > 0));
  const idle = waits.filter((w) => w.otherBytes < IDLE_LINK_BYTES).map((w) => w.ttfb);
  const base = idle.length >= 20 ? idle : ttfbs;
  const baseMedian = median(base);
  const extras = waits
    .filter((w) => w.otherBytes >= IDLE_LINK_BYTES)
    .map((w) => w.ttfb - baseMedian);
  const extraMs = median(extras);
  const wireBytes =
    bandwidth && extraMs > 0
      ? Math.round(Math.min(8_000_000, Math.max(16_384, (extraMs * bandwidth) / 1000)))
      : 0;
  return {
    staticTtfbMs: round(baseMedian),
    staticTtfbPoints: quantilePoints(base),
    wireBytes,
    rttMs: round(quantile(ttfbs, 0.1)),
    bandwidthBps: bandwidth == null ? null : Math.round(bandwidth),
    estimates: {
      staticSamples: ttfbs.length,
      idleLinkSamples: idle.length,
      staticTtfbAllMedianMs: round(median(ttfbs)),
      staticTtfbP90Ms: round(quantile(ttfbs, 0.9)),
      loadedExtraTtfbMs: round(extraMs),
      peakWindowBps: round(median(perRun.map((x) => x.window))),
      fastestSingleBps: round(median(perRun.map((x) => x.single))),
    },
  };
}

// --- archive ---------------------------------------------------------------------

/**
 * Share of the HTML (decoded bytes) that arrived within `windowMs` of its
 * headers. A host that flushes the head early lets the page start fetching
 * before the rest of the HTML arrives.
 */
export function earlyFraction(doc, windowMs = 50) {
  if (!doc?.chunks?.length || typeof doc.headersEndMs !== 'number') return null;
  const total = doc.chunks.reduce((s, c) => s + c.bytes, 0);
  if (!total) return null;
  return (
    doc.chunks.filter((c) => c.ms <= doc.headersEndMs + windowMs).reduce((s, c) => s + c.bytes, 0) /
    total
  );
}

function mostCommon(values) {
  const counts = new Map();
  for (const v of values.filter((x) => x != null)) counts.set(v, (counts.get(v) ?? 0) + 1);
  return [...counts].sort((a, b) => b[1] - a[1])[0]?.[0] ?? null;
}

const ttfbOf = (r) => (done(r) ? r.headersEndMs - r.sendStartMs : null);
const downloadOf = (r) => (done(r) ? r.endMs - r.headersEndMs : null);
const replayable = (r) => isHttp(r.url) && !r.failed && !r.blocked && typeof r.status === 'number';

/**
 * The page's own document: the row of the navigation entry, else the first
 * document. Other documents (iframes) are recorded like any other request.
 */
const mainDocument = (rows) =>
  rows.find((r) => r.navigation) ?? rows.find((r) => r.type === 'Document');

/** Requests grouped by key, in call order. */
function callsByKey(requests, volatile) {
  const map = new Map();
  const main = mainDocument(requests);
  for (const r of requests) {
    if (!replayable(r) || r === main) continue;
    const key = requestKey(r.method, r.url, volatile);
    map.set(key, [...(map.get(key) ?? []), r]);
  }
  return map;
}

const medianOf = (values) => round(median(values.filter((v) => typeof v === 'number')));

/**
 * The replay archive. `timing` = timing runs ({requests, lcp, criticalPath, path},
 * requests with raw URLs). `bodyRun` = one more load whose rows also carry
 * safelisted response headers, a body reference and request body hashes.
 * Responses come from the body run; times are medians over the timing runs,
 * matched by key and call order.
 */
export function buildArchive({ config, variant, timing, bodyRun, recordedAtMs = Date.now() }) {
  const volatile = findVolatileParams([...timing.map((t) => t.requests), bodyRun.rows]);
  const timingCalls = timing.map((t) => callsByKey(t.requests, volatile));
  const endpoints = {};
  for (const [key, calls] of callsByKey(bodyRun.rows, volatile)) {
    const [method] = key.split(' ', 1);
    endpoints[key] = {
      method,
      calls: calls.map((r, i) => {
        const samples = timingCalls.map((m) => m.get(key)?.[i]).filter(Boolean);
        return {
          url: r.url,
          type: r.type,
          status: r.status,
          headers: r.headers ?? {},
          bodyRef: r.bodyRef ?? null,
          bodyBytes: r.bodyBytes ?? 0,
          encodedBytes: r.encodedBytes ?? null,
          reqBodyHash: r.reqBodyHash ?? null,
          reqBodyNormHash: r.reqBodyNormHash ?? null,
          ttfbMs: medianOf(samples.length ? samples.map(ttfbOf) : [ttfbOf(r)]),
          downloadMs: medianOf(samples.length ? samples.map(downloadOf) : [downloadOf(r)]),
          samples: samples.length,
        };
      }),
    };
  }
  const mainRow = mainDocument(bodyRun.rows);
  const docRow = mainRow && replayable(mainRow) ? mainRow : null;
  const docTimes = timing.map((t) => mainDocument(t.requests)).filter((r) => r && !r.failed);
  const assetSizes = new Map();
  for (const t of timing) {
    for (const r of t.requests) {
      if (!isStaticRequest(r, config.origin) || !done(r)) continue;
      const p = new URL(r.url).pathname;
      assetSizes.set(p, [...(assetSizes.get(p) ?? []), r.encodedBytes]);
    }
  }
  const encodingOf = new Map(
    bodyRun.rows
      .filter((r) => isStaticRequest(r, config.origin))
      .map((r) => [new URL(r.url).pathname, r.headers?.['content-encoding'] ?? null]),
  );
  const hosts = new Map();
  for (const r of bodyRun.rows) {
    if (!isHttp(r.url)) continue;
    const h = hosts.get(new URL(r.url).host) ?? {
      host: new URL(r.url).host,
      requests: 0,
      blocked: 0,
    };
    h.requests++;
    if (r.blocked) h.blocked++;
    hosts.set(h.host, h);
  }
  const breakdowns = timing.map((t) => t.criticalPath?.breakdown).filter(Boolean);
  const pick = (k) => medianOf(breakdowns.map((b) => b[k]));
  return {
    schemaVersion: ARCHIVE_SCHEMA,
    kind: 'perf-kit-replay',
    recordedAtMs,
    url: variant.url,
    origin: config.origin,
    variant: variant.name,
    build: variant.build ?? null,
    gate: config.gate,
    block: config.block,
    ignoreLcp: config.ignoreLcp,
    redact: config.redact,
    volatile,
    document: docRow
      ? {
          url: docRow.url,
          status: docRow.status,
          headers: docRow.headers ?? {},
          bodyRef: docRow.bodyRef ?? null,
          ttfbMs: medianOf(docTimes.map(ttfbOf)),
          downloadMs: medianOf(docTimes.map(downloadOf)),
          earlyFraction: (() => {
            const f = median(
              docTimes.map((d) => earlyFraction(d)).filter((v) => typeof v === 'number'),
            );
            return f == null ? null : Math.round(f * 1000) / 1000;
          })(),
        }
      : null,
    endpoints,
    assets: [...assetSizes].map(([p, sizes]) => ({
      path: p,
      encodedBytes: round(median(sizes)),
      encoding: encodingOf.get(p) ?? null,
    })),
    network: {
      ...fitNetworkModel(
        timing.map((t) => t.requests),
        config.origin,
      ),
      // Chrome's own rating of the network. At 3g and slower it holds back
      // low-priority requests even on HTTP/2; the replay forces the same rating.
      effectiveType: mostCommon(timing.map((t) => t.connection?.effectiveType)),
      connectionRttMs: medianOf(timing.map((t) => t.connection?.rttMs)),
    },
    field: {
      runs: timing.length,
      paths: [...new Set(timing.map((t) => t.path))],
      lcpMs: medianOf(timing.map((t) => t.lcp)),
      documentEndMs: pick('documentEndMs'),
      beforeGateMs: pick('beforeGateMs'),
      gateAfterDocumentMs: pick('gateAfterDocumentMs'),
      ttfbMs: pick('ttfbMs'),
      downloadMs: pick('downloadMs'),
      afterGateMs: pick('afterGateMs'),
    },
    hosts: [...hosts.values()].sort((a, b) => b.requests - a.requests),
    page: bodyRun.page ?? null,
  };
}

// --- matching --------------------------------------------------------------------

/**
 * Recorded responses by key, served in call order within one page load
 * (`reset()` starts a new one). Levels, best first:
 *   exact            same URL as the recorded call
 *   volatile         only volatile parameters differ
 *   body             a request body: same bytes as a recorded call
 *   body-normalized  same bytes after time stamps and IDs are replaced
 *   order            a request body that matches no recorded body: next unused call
 *   repeat           more calls than recorded: the last (or the same-body) response again
 *   miss             no recorded request with this key - the server answers 404
 */
export function createResponseIndex(endpoints, volatile = {}) {
  let used = new Map();
  return {
    reset() {
      used = new Map();
    },
    match({ method, url, body = null }) {
      const key = requestKey(method, url, volatile);
      const calls = endpoints[key]?.calls;
      if (!calls?.length) return { key, call: null, level: 'miss' };
      const taken = used.get(key) ?? new Set();
      used.set(key, taken);
      const free = calls.map((_, i) => i).filter((i) => !taken.has(i));
      let pick = null;
      let level = null;
      const { bodyHash, bodyNormHash } = bodyHashes(body);
      if (bodyHash) {
        const tries = [
          [free.find((i) => calls[i].reqBodyHash === bodyHash), 'body'],
          [free.find((i) => calls[i].reqBodyNormHash === bodyNormHash), 'body-normalized'],
          [calls.findLastIndex((c) => c.reqBodyHash === bodyHash), 'repeat'],
          [free[0], 'order'],
        ];
        [pick, level] = tries.find(([i]) => i != null && i !== -1) ?? [null, null];
      } else if (free.length) {
        pick = free[0];
        level = calls[pick].url === url ? 'exact' : 'volatile';
      }
      if (pick == null) {
        pick = calls.length - 1;
        level = 'repeat';
      }
      taken.add(pick);
      return { key, call: calls[pick], level };
    },
  };
}

// --- document --------------------------------------------------------------------
// A host often changes the HTML it serves: a CSP nonce on every script, an
// analytics snippet, a service worker or bot-check loader. Those tags load and
// run during startup, so a replay of the bare dist index.html is too fast.

const ELEMENT_RE = /<(script|style)\b[^>]*>[\s\S]*?<\/\1\s*>|<(?:link|meta|base)\b[^>]*>/gi;
const stripNonce = (html) => html.replace(/\snonce\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, '');

function attrOf(tag, name) {
  const m = new RegExp(`\\s${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, 'i').exec(tag);
  return m ? (m[1] ?? m[2] ?? m[3]) : null;
}

// Content hashes in file names change with every build: /assets/app.Ab12Cd34.js -> /assets/app.<hash>.js
const refShape = (ref) => ref.replace(/[.-][A-Za-z0-9_-]{8,}(?=\.[a-z0-9]+$)/, '.<hash>');

function documentElements(html) {
  return [...html.matchAll(ELEMENT_RE)].map((m) => {
    const open = m[0].slice(0, m[0].indexOf('>') + 1);
    const id = attrOf(open, 'id');
    const ref = attrOf(open, 'src') ?? attrOf(open, 'href');
    const tag = /^<([a-z]+)/i.exec(open)[1].toLowerCase();
    return {
      text: m[0],
      end: m.index + m[0].length,
      tag,
      ref,
      identity: id ? `id:${id}` : ref ? `ref:${refShape(ref)}` : `inline:${sha256(m[0])}`,
    };
  });
}

/**
 * The local index.html plus every tag that only the recorded HTML has, each put
 * after the tag it followed in the recorded HTML. Never added: tags that point
 * into a folder the local index.html loads its own files from (/assets/...):
 * those are files of the recorded build. A tag with the same id in both files
 * keeps the local version (build metadata, the entry script).
 */
export function mergeDocument(recordedHtml, localHtml) {
  const recorded = documentElements(stripNonce(recordedHtml));
  const local = documentElements(localHtml);
  const localIds = new Set(local.map((e) => e.identity));
  const dirOf = (ref) => ref.slice(0, ref.lastIndexOf('/') + 1);
  const assetDirs = new Set(local.filter((e) => e.ref?.startsWith('/')).map((e) => dirOf(e.ref)));
  const after = new Map(); // local identity (or null = top of <head>) -> tags to add after it
  const added = [];
  let anchor = null;
  for (const e of recorded) {
    if (localIds.has(e.identity)) {
      anchor = e.identity;
      continue;
    }
    if (e.ref?.startsWith('/') && assetDirs.has(dirOf(e.ref))) continue;
    after.set(anchor, [...(after.get(anchor) ?? []), e.text]);
    added.push({
      tag: e.tag,
      ref: e.ref ? refShape(e.ref) : null,
      inlineChars: e.ref ? 0 : e.text.length,
    });
  }
  if (!added.length) return { html: localHtml, added };
  let html = '';
  let pos = 0;
  const head = /<head\b[^>]*>/i.exec(localHtml);
  const top = head ? head.index + head[0].length : 0;
  const cuts = [{ at: top, texts: after.get(null) ?? [] }];
  const placed = new Set([null]);
  for (const e of local) {
    if (placed.has(e.identity) || !after.has(e.identity)) continue;
    placed.add(e.identity);
    cuts.push({ at: e.end, texts: after.get(e.identity) });
  }
  for (const cut of cuts.sort((a, b) => a.at - b.at)) {
    html += localHtml.slice(pos, cut.at) + cut.texts.join('');
    pos = cut.at;
  }
  return { html: html + localHtml.slice(pos), added };
}

// --- calibration -----------------------------------------------------------------

/**
 * Replay of the recorded build against the field medians. A metric passes when
 * the replay median is within max(relative x field, floorMs) of the field median.
 */
export function calibrate(field, replay, { relative = 0.15, floorMs = 150 } = {}) {
  const rows = [
    ['lcpMs', 'LCP'],
    ['documentEndMs', 'document end'],
    ['beforeGateMs', 'gate start'],
    ['gateAfterDocumentMs', 'gate - doc'],
    ['ttfbMs', 'gate ttfb'],
    ['afterGateMs', 'after gate'],
  ].map(([key, label]) => {
    const f = field[key];
    const r = replay[key];
    if (typeof f !== 'number') return { key, label, field: null, replay: r ?? null, pass: null };
    // The field has the value and the replay does not (no gate, no LCP): a model error.
    if (typeof r !== 'number') return { key, label, field: f, replay: null, pass: false };
    const toleranceMs = Math.round(Math.max(relative * Math.abs(f), floorMs));
    return {
      key,
      label,
      field: f,
      replay: r,
      deltaMs: Math.round(r - f),
      toleranceMs,
      pass: Math.abs(r - f) <= toleranceMs,
    };
  });
  const judged = rows.filter((r) => r.pass !== null);
  return { rows, pass: judged.length > 0 && judged.every((r) => r.pass) };
}

/**
 * Requests that at least half of the timing runs made but that the archive has no
 * response for: the replay answers them with a 404. Files the local build serves
 * (`inBuild`) are not gaps. A request with a new URL on every load (a bot check)
 * is in one run only, so it is not a gap either.
 */
export function archiveGaps(archive, runs, { inBuild = () => false } = {}) {
  const counts = new Map();
  for (const run of runs) {
    const keys = new Set();
    const main = mainDocument(run.requests ?? []);
    for (const r of run.requests ?? []) {
      if (!replayable(r) || r === main || inBuild(r)) continue;
      keys.add(requestKey(r.method, r.url, archive.volatile));
    }
    for (const k of keys) counts.set(k, (counts.get(k) ?? 0) + 1);
  }
  const need = Math.ceil(runs.length / 2);
  return [...counts]
    .filter(([key, n]) => n >= need && !archive.endpoints[key])
    .map(([key, n]) => ({ key, runs: n, of: runs.length }))
    .sort((a, b) => b.runs - a.runs || a.key.localeCompare(b.key));
}

/**
 * What a calibration asks for next:
 *   record-again - the archive misses responses that most field runs got; a new
 *                  recording (with a longer --settle) can fix it
 *   model-error  - the archive is complete but a row is off; a new recording
 *                  does not fix it, the replay model must change
 *   pass         - every row is within tolerance and the archive is complete
 */
export function calibrationVerdict(calibration, gaps = []) {
  if (gaps.length) return 'record-again';
  return calibration.pass ? 'pass' : 'model-error';
}
