// One field navigation, in three kinds. They stay separate on purpose: precise
// coverage, the sampling profiler and body buffers slow the page down, so a run
// that collects them cannot also be a timing run.
//   timingRun     - web vitals, every request with CDP timing and initiator stack
//   diagnosticRun - V8 precise coverage at a cut point and at settle, a CPU
//                   profile to the cut, and the body of each same-origin script
//                   (for build matching)
//   bodyRun       - every response body, response headers and request bodies,
//                   for a replay archive (record command)

import { sleep } from '../cdp.mjs';
import { COLLECT_JS, OBSERVER_JS } from '../measure.mjs';
import { openFieldPage } from './browser.mjs';

/** Every LCP entry (the lab observer keeps only the last), page visibility. */
export function fieldObserverJs(ignoreSelector) {
  return `(() => {
  const F = (window.__fieldMetrics = { lcp: [], hidden: document.visibilityState === 'hidden' });
  const sel = ${JSON.stringify(ignoreSelector ?? null)};
  addEventListener('visibilitychange', () => { if (document.visibilityState === 'hidden') F.hidden = true; }, true);
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        let ignored = false;
        try { ignored = Boolean(sel && e.element && e.element.closest(sel)); } catch {}
        F.lcp.push({ t: e.startTime, size: e.size, ignored });
      }
    }).observe({ type: 'largest-contentful-paint', buffered: true });
  } catch {}
})();`;
}

const STATE_JS = `(() => {
  const nav = performance.getEntriesByType('navigation')[0];
  const F = window.__fieldMetrics || { lcp: [], hidden: false };
  const counted = F.lcp.filter((e) => !e.ignored);
  return {
    load: nav ? nav.loadEventEnd : 0,
    lcp: counted.length ? counted[counted.length - 1].t : null,
    fcp: (window.__perfMetrics || {}).fcp ?? null,
    hidden: F.hidden,
  };
})()`;

const PAGE_INFO_JS = `(() => {
  const nav = performance.getEntriesByType('navigation')[0];
  const F = window.__fieldMetrics || { lcp: [] };
  const entry = [...document.querySelectorAll('script[type="module"][src]')]
    .map((s) => new URL(s.src, location.href))
    .find((u) => u.origin === location.origin);
  const c = navigator.connection;
  return {
    path: location.pathname,
    navUrl: nav ? nav.name.split('#')[0] : null,
    requestStart: nav ? nav.requestStart : null,
    lcpEntries: F.lcp,
    hidden: F.hidden,
    entry: entry ? entry.pathname : null,
    connection: c ? { effectiveType: c.effectiveType, rttMs: c.rtt, downlinkMbps: c.downlink } : null,
  };
})()`;

/**
 * Requests with CDP timing, in CDP seconds until `finish` converts them to page
 * time. Keeps URL, method, type, priority and the initiator's call frames - never
 * headers, cookies or bodies. For the document it also keeps when its bytes
 * arrived (sizes only): a host that flushes the head early starts the page
 * earlier than one that sends the HTML at once.
 */
export function recordFieldNetwork(page, { maxFrames = 8 } = {}) {
  const byId = new Map();
  const order = [];
  const stops = [
    page.on('Network.requestWillBeSent', (e) => {
      const frames = [];
      for (let st = e.initiator?.stack; st && frames.length < maxFrames; st = st.parent) {
        for (const f of st.callFrames ?? []) {
          if (f.url && frames.length < maxFrames) {
            frames.push({
              url: f.url,
              functionName: f.functionName,
              lineNumber: f.lineNumber,
              columnNumber: f.columnNumber,
            });
          }
        }
      }
      const previous = byId.get(e.requestId);
      const row = {
        id: e.requestId,
        url: e.request.url.split('#')[0],
        method: e.request.method,
        type: e.type ?? null,
        ts: e.timestamp,
        priority: e.request.initialPriority ?? null,
        redirected: Boolean(e.redirectResponse) || Boolean(previous?.redirected),
        initiator: {
          type: e.initiator?.type ?? null,
          url: frames[0]?.url ?? e.initiator?.url ?? null,
          frames,
        },
      };
      if (!previous) order.push(e.requestId);
      byId.set(e.requestId, row);
    }),
    page.on('Network.responseReceived', (e) => {
      const r = byId.get(e.requestId);
      if (!r) return;
      r.status = e.response.status;
      r.protocol = e.response.protocol ?? null;
      r.type = e.type ?? r.type;
      const t = e.response.timing;
      if (t) {
        r.sendStart = t.requestTime + t.sendStart / 1000;
        r.headersEnd = t.requestTime + t.receiveHeadersEnd / 1000;
      }
    }),
    page.on('Network.resourceChangedPriority', (e) => {
      const r = byId.get(e.requestId);
      if (r) r.lastPriority = e.newPriority;
    }),
    page.on('Network.dataReceived', (e) => {
      const r = byId.get(e.requestId);
      if (r?.type === 'Document') (r.chunks ??= []).push({ ts: e.timestamp, bytes: e.dataLength });
    }),
    page.on('Network.loadingFinished', (e) => {
      const r = byId.get(e.requestId);
      if (!r) return;
      r.end = e.timestamp;
      r.encodedBytes = e.encodedDataLength;
    }),
    page.on('Network.loadingFailed', (e) => {
      const r = byId.get(e.requestId);
      if (!r) return;
      r.end = e.timestamp;
      r.failed = e.errorText;
      r.blocked = e.blockedReason ?? null;
    }),
  ];
  return {
    rows: () => order.map((id) => byId.get(id)),
    stop: () => stops.forEach((stop) => stop()),
    /**
     * Page time (ms since timeOrigin, the clock LCP uses) for every request. The
     * offset comes from the document request: its CDP sendStart equals the
     * navigation entry's requestStart. After a client-side redirect the page is
     * the second document, so `navUrl` (the navigation entry's URL) picks it; that
     * row is marked `navigation`.
     */
    finish(requestStartPageMs, redact, navUrl = null) {
      const rows = order.map((id) => byId.get(id));
      const docs = rows.filter((r) => r.type === 'Document' && r.sendStart != null);
      const doc = docs.find((r) => r.url === navUrl) ?? docs[0];
      const offset =
        doc && typeof requestStartPageMs === 'number'
          ? doc.sendStart * 1000 - requestStartPageMs
          : rows[0]
            ? rows[0].ts * 1000
            : 0;
      const pt = (s) => (typeof s === 'number' ? Math.round((s * 1000 - offset) * 10) / 10 : null);
      return rows.map((r) => ({
        url: redact(r.url),
        method: r.method,
        type: r.type,
        status: r.status ?? null,
        protocol: r.protocol ?? null,
        priority: r.priority,
        lastPriority: r.lastPriority ?? r.priority,
        startMs: pt(r.ts),
        sendStartMs: pt(r.sendStart),
        headersEndMs: pt(r.headersEnd),
        endMs: pt(r.end),
        encodedBytes: r.encodedBytes ?? null,
        failed: r.failed ?? null,
        blocked: r.blocked ?? null,
        redirected: r.redirected,
        ...(r === doc ? { navigation: true } : {}),
        ...(r.chunks ? { chunks: r.chunks.map((c) => ({ ms: pt(c.ts), bytes: c.bytes })) } : {}),
        initiator: {
          type: r.initiator.type,
          url: redact(r.initiator.url),
          frames: r.initiator.frames.map((f) => ({ ...f, url: redact(f.url) })),
        },
      }));
    },
  };
}

async function waitFor(page, predicate, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const state = await page.evaluate(STATE_JS).catch(() => null);
    if (state && (predicate(state) || state.hidden)) return state;
    if (Date.now() > deadline) return state;
    await sleep(25);
  }
}

/** The lab sample shape, without the text an LCP element shows (real account data). */
function scrubSample(sample, redact) {
  const out = { ...sample };
  out.resources = (sample.resources ?? []).map((r) => ({ ...r, name: redact(r.name) }));
  if (out.lcpElement) {
    const { text, finalText, ...rest } = out.lcpElement;
    out.lcpElement = {
      ...rest,
      url: redact(rest.url),
      finalUrl: redact(rest.finalUrl),
      textLength: (finalText ?? text ?? '').length,
    };
  }
  if (out.blockingGate?.worst) {
    out.blockingGate = {
      ...out.blockingGate,
      worst: out.blockingGate.worst.map((w) => ({ ...w, name: redact(w.name) })),
    };
  }
  delete out.heroTitle;
  delete out.heroSubtitle;
  return out;
}

/** `keepRaw` adds `rawRequests`: the same rows with raw URLs (archives, chunk names). */
export async function timingRun(
  cdp,
  {
    variant,
    config,
    context,
    saved,
    configured,
    redact,
    settleMs,
    timeoutMs,
    throttle,
    keepRaw = false,
  },
) {
  const page = await openFieldPage(cdp, {
    browserContextId: context,
    origin: config.origin,
    cookies: variant.cookies,
    saved,
    configured,
    block: config.block,
    scripts: [OBSERVER_JS, fieldObserverJs(config.ignoreLcp)],
    throttle,
  });
  const net = recordFieldNetwork(page);
  try {
    await page.navigate(variant.url);
    await waitFor(page, (s) => s.load > 0 && s.lcp != null, timeoutMs);
    await sleep(settleMs);
    const raw = await page.evaluate(COLLECT_JS);
    const info = await page.evaluate(PAGE_INFO_JS);
    const counted = info.lcpEntries.filter((e) => !e.ignored);
    const sample = scrubSample(raw, redact);
    // The lab observer keeps the LAST entry, which may sit in an ignored region
    // (a splash/loader). Field LCP is the last entry outside it.
    sample.lcp = counted.length ? counted[counted.length - 1].t : null;
    sample.lcpIgnoredEntries = info.lcpEntries.length - counted.length;
    const requests = net.finish(info.requestStart, redact, info.navUrl);
    let invalid = null;
    if (info.hidden) invalid = 'page was hidden (window covered or in the background)';
    else if (sample.lcp == null) invalid = 'no LCP outside the ignored region';
    return {
      sample,
      requests,
      ...(keepRaw ? { rawRequests: net.finish(info.requestStart, (v) => v, info.navUrl) } : {}),
      path: redact(info.path),
      entry: info.entry,
      connection: info.connection,
      invalid,
    };
  } finally {
    net.stop();
    await page.close().catch(() => {});
  }
}

/**
 * Recording run for a replay archive. Returns every request (page time, raw URL)
 * with its response headers, decoded response body and request body. The caller
 * keeps only safelisted headers and body hashes, and writes the bodies outside
 * the repo. Nothing here is printed. Page state that the replay cannot copy is
 * described by names and counts only: script-visible cookie names and the
 * number of localStorage keys.
 */
export async function bodyRun(
  cdp,
  { variant, config, context, saved, configured, settleMs, timeoutMs },
) {
  const page = await openFieldPage(cdp, {
    browserContextId: context,
    origin: config.origin,
    cookies: variant.cookies,
    saved,
    configured,
    block: config.block,
    scripts: [OBSERVER_JS, fieldObserverJs(config.ignoreLcp)],
    bodies: true,
  });
  const net = recordFieldNetwork(page);
  const extra = new Map();
  const of = (id) => extra.get(id) ?? extra.set(id, {}).get(id);
  const stops = [
    page.on('Network.requestWillBeSent', (e) => {
      const x = of(e.requestId);
      x.postData = e.request.postData ?? null;
      x.hasPostData = Boolean(e.request.hasPostData);
    }),
    page.on('Network.responseReceived', (e) => {
      of(e.requestId).headers = e.response.headers;
    }),
  ];
  try {
    await page.navigate(variant.url);
    await waitFor(page, (s) => s.load > 0 && s.lcp != null, timeoutMs);
    await sleep(settleMs);
    const info = await page.evaluate(PAGE_INFO_JS);
    const state = await page
      .evaluate(
        `({ cookieNames: document.cookie.split(';').map((c) => c.split('=')[0].trim()).filter(Boolean),
            localStorageKeys: (() => { try { return localStorage.length; } catch { return null; } })() })`,
      )
      .catch(() => ({ cookieNames: [], localStorageKeys: null }));
    const raw = net.rows();
    const timed = net.finish(info.requestStart, (v) => v, info.navUrl);
    const rows = [];
    for (let i = 0; i < raw.length; i++) {
      const x = extra.get(raw[i].id) ?? {};
      let body = null;
      if (!raw[i].failed && raw[i].end != null && /^https?:/.test(raw[i].url)) {
        try {
          const res = await page.send('Network.getResponseBody', { requestId: raw[i].id });
          body = Buffer.from(res.body, res.base64Encoded ? 'base64' : 'utf8');
        } catch {
          /* no body (204, redirect) or evicted from the buffer */
        }
      }
      let postData = x.postData ?? null;
      if (postData == null && x.hasPostData) {
        postData = await page
          .send('Network.getRequestPostData', { requestId: raw[i].id })
          .then((r) => r.postData)
          .catch(() => null);
      }
      rows.push({ ...timed[i], headers: x.headers ?? {}, body, postData });
    }
    let invalid = null;
    if (info.hidden) invalid = 'page was hidden (window covered or in the background)';
    return { rows, page: { path: info.path, entry: info.entry, ...state }, invalid };
  } finally {
    stops.forEach((stop) => stop());
    net.stop();
    await page.close().catch(() => {});
  }
}

/**
 * Coverage + CPU profile run. Cut points:
 *   fcp  - first contentful paint (the lab's cut)
 *   lcp  - the first LCP entry outside the ignored region
 *   gate - the moment the config's gate request is sent: "what ran before the
 *          request the LCP waits for". The snapshot is async, so a few ms of
 *          later execution can land in it.
 * Coverage is armed on about:blank and survives the navigation; if the entry
 * script is missing from the result, the caller reports it.
 */
export async function diagnosticRun(
  cdp,
  { variant, config, context, saved, configured, redact, cut, settleMs, timeoutMs, throttle },
) {
  const page = await openFieldPage(cdp, {
    browserContextId: context,
    origin: config.origin,
    cookies: variant.cookies,
    saved,
    configured,
    block: config.block,
    scripts: [OBSERVER_JS, fieldObserverJs(config.ignoreLcp)],
    bodies: true,
    throttle,
  });
  const net = recordFieldNetwork(page, { maxFrames: 64 });
  let gateSeen = false;
  // The timing runs find the gate in redacted URLs, so the cut matches the same way.
  const gate = redact(config.gate);
  const stopGate =
    cut === 'gate' && gate
      ? page.on('Network.requestWillBeSent', (e) => {
          if (redact(e.request.url).includes(gate)) gateSeen = true;
        })
      : () => {};
  // With the debugger on, a `debugger` statement or a breakpoint stops the page
  // until a client resumes it. Resume at once: Debugger.setSkipAllPauses does
  // not survive the navigation.
  const stopPauses = page.on('Debugger.paused', () => {
    page.send('Debugger.resume').catch(() => {});
  });
  try {
    // Async parent stacks make the initiator of a fetch name its real caller,
    // not only the wrapper or microtask that issued it. They cost CPU, which is
    // one more reason this run never feeds timing numbers.
    await page.send('Debugger.enable');
    await page.send('Debugger.setAsyncCallStackDepth', { maxDepth: 32 });
    await page.send('Profiler.enable');
    await page.send('Profiler.setSamplingInterval', { interval: 200 });
    await page.send('Profiler.startPreciseCoverage', { callCount: false, detailed: true });
    await page.send('Profiler.start');
    await page.navigate(variant.url);
    const deadline = Date.now() + timeoutMs;
    let state = null;
    let reached = false;
    for (;;) {
      state = await page.evaluate(STATE_JS).catch(() => null);
      reached = cut === 'gate' ? gateSeen : cut === 'fcp' ? state?.fcp != null : state?.lcp != null;
      if (reached || state?.hidden || Date.now() > deadline) break;
      await sleep(cut === 'gate' ? 5 : 25);
    }
    const atCut = await page.send('Profiler.takePreciseCoverage');
    const cutMs = await page.evaluate('performance.now()');
    const { profile } = await page.send('Profiler.stop');
    await waitFor(page, (s) => s.load > 0 && s.lcp != null, Math.max(0, deadline - Date.now()));
    await sleep(settleMs);
    const atSettle = await page.send('Profiler.takePreciseCoverage');
    await page.send('Profiler.stopPreciseCoverage').catch(() => {});
    const info = await page.evaluate(PAGE_INFO_JS);
    const requests = net.finish(info.requestStart, redact, info.navUrl);

    // V8 omits a script from a snapshot when it ran nothing new in that window.
    const byUrl = new Map();
    const collect = (take, key) => {
      for (const s of take.result) {
        if (!s.url || !s.url.startsWith(config.origin)) continue;
        const rec = byUrl.get(s.url) ?? {
          url: s.url,
          pathname: new URL(s.url).pathname,
          atPaint: null,
          atSettle: null,
        };
        rec[key] = (rec[key] ?? []).concat(s.functions);
        byUrl.set(s.url, rec);
      }
    };
    collect(atCut, 'atPaint');
    collect(atSettle, 'atSettle');

    // Bodies for build matching: only same-origin scripts that executed.
    const rows = net.rows();
    const bodies = new Map();
    for (const url of byUrl.keys()) {
      if (!/\.[mc]?js$/.test(new URL(url).pathname)) continue;
      const row = rows.find((r) => r.url === url && !r.failed);
      if (!row) continue;
      try {
        const { body, base64Encoded } = await page.send('Network.getResponseBody', {
          requestId: row.id,
        });
        bodies.set(url, base64Encoded ? Buffer.from(body, 'base64').toString('utf8') : body);
      } catch {
        /* evicted from the buffer: chunk-level coverage still works, matching does not */
      }
    }
    // Raw URLs: a redacted chunk name (a long hex hash) would not match its script.
    const scriptFetches = net
      .finish(info.requestStart, (v) => v, info.navUrl)
      .filter((r) => r.url.startsWith(config.origin) && /\.[mc]?js$/.test(new URL(r.url).pathname))
      .map((r) => ({
        pathname: new URL(r.url).pathname,
        startMs: r.startMs,
        endMs: r.endMs,
        bytes: r.encodedBytes ?? 0,
        prePaint: typeof r.startMs === 'number' && r.startMs < cutMs,
      }));
    let invalid = null;
    if (info.hidden || state?.hidden)
      invalid = 'page was hidden (window covered or in the background)';
    else if (cut === 'gate' && !reached)
      invalid = `gate request "${gate}" was not sent within ${timeoutMs}ms`;
    else if (!reached) invalid = `no ${cut.toUpperCase()} within ${timeoutMs}ms`;
    return {
      scripts: [...byUrl.values()],
      bodies,
      requests,
      scriptFetches,
      profile: { ...profile, origin: config.origin },
      cut,
      cutMs,
      entry: info.entry,
      path: redact(info.path),
      invalid,
    };
  } finally {
    await page.send('Debugger.disable').catch(() => {});
    stopGate();
    stopPauses();
    net.stop();
    await page.close().catch(() => {});
  }
}
