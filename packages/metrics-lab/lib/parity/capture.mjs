// Deterministic page capture for parity: the browser half. One capture = one fresh
// tab that loads a page under a pinned clock and seeded Math.random, stills every
// motion source, waits for the page to stop changing, and records what it renders -
// viewport and full-page screenshots, element boxes + computed styles, visible
// text, images, the fonts Chrome actually drew with, accessible names, and any
// runtime errors on the way.
//
// The stabilization sequence is the one hosted visual-regression tools converged on
// (Chromatic, Percy, and Cloudflare's Delta - see its src/lib/capture.ts): reduced
// motion, a CSS kill switch for animations/transitions, fonts.ready, rAF-idle and
// DOM-idle windows under ONE shared deadline, then freezing Web Animations and SMIL
// right before each screenshot. What it can't still (a live clock, network races)
// shows up as the unstable mask in compare.mjs instead of as a false difference.

import { openPage, sleep } from '../cdp.mjs';
import { ELEMENT_PATH_JS } from '../measure.mjs';

export const VIEWPORT = { width: 1280, height: 900 };
/** Full-page captures stop at this many viewport heights (and say so). */
export const FULL_PAGE_MAX_SCREENS = 10;

// Two capture conditions. The candidate always renders under A; the original renders
// under A and again under B. B moves the clock by 1d1h1m1s and reseeds Math.random,
// so anything driven by time or randomness differs between the two originals and
// lands in the unstable mask - it is never judged, rather than judged wrongly when a
// deferral reorders Math.random calls.
export const CONDITION_A = { seed: 0x2545f491, timeOrigin: Date.UTC(2026, 0, 15, 12, 0, 0) };
export const CONDITION_B = {
  seed: 0x6c8e9cf5,
  timeOrigin: CONDITION_A.timeOrigin + 86_400_000 + 3_600_000 + 60_000 + 1000,
};

const NAV_TIMEOUT_MS = 30_000;
const SETTLE_MS = 5000;
const STEP_SETTLE_MS = 600;
const RETURN_SETTLE_MS = 3000;
const NETWORK_QUIET_MS = 300;
// A finite JS animation (a chart easing in) stops requesting frames within this;
// a loop still running after it is infinite (a Rive/canvas hero, a spinner) and the
// rest of the capture stops waiting for it - the freeze and the unstable mask cover
// whatever it draws.
const RAF_CAP_MS = 2000;
const CAPTURE_TIMEOUT_MS = 120_000;
const FONT_ELEMENT_LIMIT = 120;

// Computed styles recorded per element. They explain a changed region ("background
// went transparent", "font-size 52px -> 16px"); pixels and text decide.
export const STYLE_PROPS = [
  'display',
  'visibility',
  'opacity',
  'position',
  'z-index',
  'color',
  'background-color',
  'background-image',
  'font-family',
  'font-size',
  'font-weight',
  'font-style',
  'line-height',
  'letter-spacing',
  'text-transform',
  'text-decoration-line',
  'text-align',
  'border-top-width',
  'border-top-style',
  'border-top-color',
  'border-radius',
  'box-shadow',
  'transform',
  'filter',
  'object-fit',
];

// Accessible names worth comparing: controls, landmarks, headings, images. Plain
// text is compared through innerText already.
const A11Y_ROLES = new Set([
  'heading',
  'link',
  'button',
  'img',
  'image',
  'textbox',
  'searchbox',
  'checkbox',
  'radio',
  'combobox',
  'listbox',
  'switch',
  'slider',
  'tab',
  'menuitem',
  'navigation',
  'banner',
  'main',
  'contentinfo',
  'complementary',
  'form',
  'search',
  'dialog',
  'alert',
  'figure',
]);

/** Pinned clock (advancing from a fixed origin, so busy-wait loops still end) + seeded Math.random. */
export function determinismScript({ seed, timeOrigin }) {
  return `(() => {
  let s = ${seed >>> 0};
  Math.random = function random() {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const RealDate = Date;
  const p0 = performance.now();
  const now = () => ${timeOrigin} + (performance.now() - p0);
  function PinnedDate(...args) {
    if (new.target === undefined) return new RealDate(now()).toString();
    return Reflect.construct(RealDate, args.length ? args : [now()], new.target);
  }
  PinnedDate.prototype = RealDate.prototype;
  PinnedDate.now = () => Math.floor(now());
  PinnedDate.parse = RealDate.parse;
  PinnedDate.UTC = RealDate.UTC;
  Object.defineProperty(PinnedDate, 'name', { value: 'Date' });
  globalThis.Date = PinnedDate;
})();`;
}

// scroll-behavior: auto keeps our own scrolling synchronous; caret-color hides a
// blinking caret in an autofocused input.
const KILL_MOTION_JS = `(() => {
  const style = document.createElement('style');
  style.setAttribute('data-metrics-lab', 'parity');
  style.textContent = '*, *::before, *::after { animation-duration: 0s !important; animation-delay: 0s !important; animation-iteration-count: 1 !important; transition-duration: 0s !important; transition-delay: 0s !important; caret-color: transparent !important; scroll-behavior: auto !important; }';
  (document.head || document.documentElement).append(style);
  return true;
})()`;

// Finite animations jump to their end state (what a visitor ends up seeing);
// infinite ones (spinners) pin at t=0. SVG SMIL is a separate timeline.
const FREEZE_JS = `(() => {
  for (const a of document.getAnimations ? document.getAnimations() : []) {
    try {
      const timing = a.effect && a.effect.getComputedTiming();
      if (timing && timing.iterations === Infinity) a.currentTime = 0;
      else {
        try { a.finish(); } catch { a.currentTime = 0; }
      }
      a.pause();
    } catch {}
  }
  for (const svg of document.querySelectorAll('svg')) {
    if (typeof svg.pauseAnimations === 'function') {
      svg.pauseAnimations();
      svg.setCurrentTime(0);
    }
  }
  return true;
})()`;

const CONTENT_HEIGHT_JS =
  'Math.max(document.documentElement.scrollHeight, document.body ? document.body.scrollHeight : 0)';

// One shared deadline across every wait (fonts, rAF-idle, DOM-idle, two paint
// frames): a page that needs all of them still settles within maxMs. Resolves to
// whether the page's animation-frame loop went quiet, so a capture stops waiting
// on an infinite one after the first time.
function settleJs(maxMs, { fonts = true, quietMs = 200, raf = true } = {}) {
  return `(async () => {
  const deadline = performance.now() + ${maxMs};
  const left = () => Math.max(0, deadline - performance.now());
  const bounded = (p) => Promise.race([p, new Promise((r) => setTimeout(r, left()))]);
  ${fonts ? 'try { await bounded(document.fonts ? document.fonts.ready : null); } catch {}' : ''}
  const quiet = (subscribe, capMs) => new Promise((resolve) => {
    const start = performance.now();
    let last = start;
    const stop = subscribe(() => { last = performance.now(); });
    const tick = () => {
      const now = performance.now();
      if (now - last >= ${quietMs}) { stop(); resolve(true); }
      else if (left() === 0 || now - start >= capMs) { stop(); resolve(false); }
      else setTimeout(tick, 50);
    };
    setTimeout(tick, 50);
  });
  // A draw loop keeps requesting frames; wrapping rAF sees it without hooking the app.
  const rafQuiet = ${raf} ? await quiet((bump) => {
    const original = window.requestAnimationFrame;
    window.requestAnimationFrame = function (cb) { bump(); return original.call(window, cb); };
    return () => { window.requestAnimationFrame = original; };
  }, ${RAF_CAP_MS}) : false;
  await quiet((bump) => {
    const observer = new MutationObserver(bump);
    observer.observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
    return () => observer.disconnect();
  }, Infinity);
  await bounded(new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  return { rafQuiet };
})()`;
}

// What the page renders, at the current scroll position. scope 'first' keeps only
// elements intersecting the viewport (the first screen); 'page' keeps everything
// plus the whole innerText. Coordinates are document coordinates.
function snapshotJs(scope, maskSelectors) {
  return `(() => {
  ${ELEMENT_PATH_JS}
  const scope = ${JSON.stringify(scope)};
  const PROPS = ${JSON.stringify(STYLE_PROPS)};
  const SKIP = new Set(['script', 'style', 'link', 'meta', 'template', 'noscript', 'title', 'base', 'br', 'option']);
  const LIMIT = scope === 'first' ? 1500 : 4000;
  const vw = innerWidth;
  const vh = innerHeight;
  const sx = scrollX;
  const sy = scrollY;
  const origin = location.origin;
  const strip = (v) => v.split(origin).join('');
  const shortSrc = (v) => (v.length > 160 ? v.slice(0, 120) + '...(' + v.length + ' chars)' : v);
  const masks = [];
  const maskedText = [];
  const maskedEls = new Set();
  const badMasks = [];
  for (const selector of ${JSON.stringify(maskSelectors)}) {
    let found;
    try { found = document.querySelectorAll(selector); } catch { badMasks.push(selector); continue; }
    for (const el of found) {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) masks.push({ x: r.left + sx, y: r.top + sy, w: r.width, h: r.height });
      maskedEls.add(el);
      for (const line of (el.innerText || '').split('\\n')) maskedText.push(line);
    }
  }
  const isMasked = (el) => {
    if (!maskedEls.size) return false;
    for (let n = el; n; n = n.parentElement) if (maskedEls.has(n)) return true;
    return false;
  };
  const offscreen = (r) => r.bottom <= 0 || r.top >= vh || r.right <= 0 || r.left >= vw;
  const elements = [];
  let truncated = false;
  for (const el of document.body ? document.body.querySelectorAll('*') : []) {
    const tag = el.tagName.toLowerCase();
    if (SKIP.has(tag) || el.hasAttribute('data-metrics-lab')) continue;
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) continue;
    if (scope === 'first' && offscreen(r)) continue;
    if (isMasked(el)) continue;
    if (elements.length >= LIMIT) { truncated = true; break; }
    const cs = getComputedStyle(el);
    const style = {};
    for (const p of PROPS) style[p] = strip(cs.getPropertyValue(p));
    let text = '';
    for (const n of el.childNodes) if (n.nodeType === 3) text += n.nodeValue;
    elements.push({
      path: pathOf(el),
      tag,
      label: labelOf(el),
      rect: { x: Math.round(r.left + sx), y: Math.round(r.top + sy), w: Math.round(r.width), h: Math.round(r.height) },
      style,
      text: text.replace(/\\s+/g, ' ').trim().slice(0, 160),
    });
  }
  const images = [];
  for (const img of document.images) {
    const r = img.getBoundingClientRect();
    if (r.width < 1 || r.height < 1 || (scope === 'first' && offscreen(r)) || isMasked(img)) continue;
    images.push(shortSrc(strip(img.currentSrc || img.src)) + ' ' + img.naturalWidth + 'x' + img.naturalHeight);
  }
  const faces = [];
  if (document.fonts) {
    for (const f of document.fonts) faces.push(f.family.replace(/["']/g, '') + ' ' + f.weight + ' ' + f.style + ': ' + f.status);
  }
  return {
    location: location.pathname + location.search,
    elements,
    truncated,
    images,
    faces,
    text: scope === 'page' && document.body ? document.body.innerText : null,
    masks,
    maskedText,
    badMasks,
    contentHeight: ${CONTENT_HEIGHT_JS},
  };
})()`;
}

function withTimeout(promise, ms, what) {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => {
      timer = setTimeout(
        () => reject(new Error(`${what} did not finish within ${ms / 1000}s`)),
        ms,
      );
    }),
  ]).finally(() => clearTimeout(timer));
}

// Idle = nothing awaiting a response and nothing received for NETWORK_QUIET_MS. A
// request that answered and went quiet counts as done even without loadingFinished:
// a streamed WebAssembly compile consumes its response and Chrome never reports
// the request finished (found via the rolldown docs' Rive hero).
function watchNetwork(page) {
  const inflight = new Map(); // requestId -> answered?
  let lastActivity = Date.now();
  const touch = (p, answered) => {
    if (answered === undefined && !inflight.has(p.requestId)) return;
    inflight.set(p.requestId, answered ?? inflight.get(p.requestId));
    lastActivity = Date.now();
  };
  page.on('Network.requestWillBeSent', (p) => touch(p, false));
  page.on('Network.responseReceived', (p) => touch(p, true));
  page.on('Network.dataReceived', (p) => touch(p));
  const finish = (p) => {
    if (inflight.delete(p.requestId)) lastActivity = Date.now();
  };
  page.on('Network.loadingFinished', finish);
  page.on('Network.loadingFailed', finish);
  return {
    // A long-poll never answers; the deadline bounds the wait.
    async idle(deadline) {
      while (Date.now() < deadline) {
        const waiting = Array.from(inflight.values()).some((answered) => !answered);
        if (!waiting && Date.now() - lastActivity >= NETWORK_QUIET_MS) return;
        await sleep(50);
      }
    },
  };
}

// Raw error text; compare.mjs normalizes it (origins, positions, hashed names) and
// only errors the original never produced count against the candidate.
function watchRuntime(page) {
  const errors = [];
  const urls = new Map();
  const argText = (a) =>
    a.value !== undefined ? String(a.value) : (a.description ?? a.unserializableValue ?? a.type);
  page.on('Network.requestWillBeSent', (p) => urls.set(p.requestId, p.request.url));
  page.on('Runtime.exceptionThrown', ({ exceptionDetails: d }) => {
    errors.push(`uncaught ${d.exception?.description ?? d.text}`);
  });
  page.on('Runtime.consoleAPICalled', ({ type, args }) => {
    if (type === 'error' || type === 'assert') {
      errors.push(`console.${type}: ${args.map(argText).join(' ')}`);
    }
  });
  // network failures arrive below with their URL; exceptions above with their text
  page.on('Log.entryAdded', ({ entry }) => {
    if (entry.level === 'error' && entry.source !== 'network' && entry.source !== 'javascript') {
      errors.push(`${entry.source}: ${entry.text}`);
    }
  });
  page.on('Network.loadingFailed', (p) => {
    if (!p.canceled) errors.push(`request failed: ${urls.get(p.requestId) ?? '?'} ${p.errorText}`);
  });
  page.on('Network.responseReceived', (p) => {
    if (p.response.status >= 400) errors.push(`HTTP ${p.response.status} ${p.response.url}`);
  });
  return errors;
}

// `motion.rafBusy` flips once a page's frame loop outlives RAF_CAP_MS; every later
// wait in the same capture skips that phase instead of burning its deadline again.
async function settle(page, net, ms, motion) {
  const deadline = Date.now() + ms;
  await net.idle(deadline);
  const result = await page.evaluate(
    settleJs(Math.max(0, deadline - Date.now()), { raf: !motion.rafBusy }),
  );
  if (!motion.rafBusy && result && !result.rafQuiet) motion.rafBusy = true;
  await net.idle(deadline);
}

// Lazy-on-visible sections only render once scrolled to; walking the page one
// viewport at a time renders them in BOTH builds before the full-page shot.
async function scrollThrough(page, net, motion) {
  const limit = VIEWPORT.height * FULL_PAGE_MAX_SCREENS;
  for (let y = VIEWPORT.height; y < limit; y += VIEWPORT.height) {
    if (y >= (await page.evaluate(CONTENT_HEIGHT_JS))) break;
    await page.evaluate(`window.scrollTo(0, ${y})`);
    const result = await page.evaluate(
      settleJs(STEP_SETTLE_MS, { fonts: false, quietMs: 100, raf: !motion.rafBusy }),
    );
    if (!motion.rafBusy && result && !result.rafQuiet) motion.rafBusy = true;
    await net.idle(Date.now() + STEP_SETTLE_MS);
  }
  await page.evaluate('window.scrollTo(0, 0)');
  await settle(page, net, RETURN_SETTLE_MS, motion);
}

async function screenshot(page, params = {}) {
  const { data } = await page.send('Page.captureScreenshot', { format: 'png', ...params });
  return Buffer.from(data, 'base64');
}

/** The fonts Chrome actually drew each first-screen text element with (fallbacks included). */
async function renderedFonts(page, elements) {
  const textEls = elements
    .filter((e) => e.text)
    .sort((a, b) => b.rect.w * b.rect.h - a.rect.w * a.rect.h)
    .slice(0, FONT_ELEMENT_LIMIT);
  if (!textEls.length) return {};
  await page.send('DOM.enable');
  await page.send('CSS.enable');
  const { root } = await page.send('DOM.getDocument', { depth: 0 });
  const out = {};
  for (const e of textEls) {
    const selector = `html > ${e.path
      .split('>')
      .map((part) => {
        const [tag, n] = part.split(':');
        return `${tag}:nth-of-type(${n})`;
      })
      .join(' > ')}`;
    try {
      const { nodeId } = await page.send('DOM.querySelector', { nodeId: root.nodeId, selector });
      if (!nodeId) continue;
      const { fonts } = await page.send('CSS.getPlatformFontsForNode', { nodeId });
      if (fonts.length) out[e.path] = fonts.map((f) => f.familyName).join(', ');
    } catch {
      // the element went away or its tag doesn't round-trip through a selector
    }
  }
  return out;
}

async function accessibleNames(page) {
  try {
    const { nodes } = await page.send('Accessibility.getFullAXTree');
    const out = [];
    for (const node of nodes) {
      const role = node.role?.value;
      const name = node.name?.value;
      if (node.ignored || !A11Y_ROLES.has(role) || typeof name !== 'string' || !name.trim()) {
        continue;
      }
      out.push(`${role} "${name.replace(/\s+/g, ' ').trim().slice(0, 100)}"`);
    }
    return out;
  } catch {
    return null;
  }
}

/**
 * Capture one page of whatever build the server currently serves. `origin` is the
 * parity server; every capture clears its storage first, so no capture sees state
 * (a "returning visitor" flag, a dismissed banner) written by an earlier one.
 */
export function capturePage(cdp, { origin, pagePath, condition, masks = [] }) {
  return withTimeout(
    captureInner(cdp, { origin, pagePath, condition, masks }),
    CAPTURE_TIMEOUT_MS,
    `capture of ${pagePath}`,
  );
}

async function captureInner(cdp, { origin, pagePath, condition, masks }) {
  const page = await openPage(cdp, { injectScript: determinismScript(condition) });
  const errors = watchRuntime(page);
  const net = watchNetwork(page);
  try {
    await page.send('Storage.clearDataForOrigin', { origin, storageTypes: 'all' }).catch(() => {});
    await page.send('Log.enable').catch(() => {});
    await page.send('Emulation.setDeviceMetricsOverride', {
      width: VIEWPORT.width,
      height: VIEWPORT.height,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await page.send('Emulation.setEmulatedMedia', {
      features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
    });
    await page.navigate(`${origin}${pagePath}`);
    const deadline = Date.now() + NAV_TIMEOUT_MS;
    for (;;) {
      const loaded = await page
        .evaluate(
          `location.origin === ${JSON.stringify(origin)} && document.readyState === 'complete'`,
        )
        .catch(() => false);
      if (loaded) break;
      if (Date.now() > deadline) throw new Error(`${pagePath} did not finish loading`);
      await sleep(50);
    }
    await page.evaluate(KILL_MOTION_JS);
    const motion = { rafBusy: false };
    await settle(page, net, SETTLE_MS, motion);
    await page.evaluate(FREEZE_JS);
    const first = await page.evaluate(snapshotJs('first', masks));
    const viewport = await screenshot(page);

    await scrollThrough(page, net, motion);
    await page.evaluate(FREEZE_JS);
    const whole = await page.evaluate(snapshotJs('page', masks));
    let full = null;
    let fullTruncated = false;
    if (whole.contentHeight > VIEWPORT.height) {
      const height = Math.min(whole.contentHeight, VIEWPORT.height * FULL_PAGE_MAX_SCREENS);
      fullTruncated = whole.contentHeight > height;
      full = await screenshot(page, {
        captureBeyondViewport: true,
        clip: { x: 0, y: 0, width: VIEWPORT.width, height, scale: 1 },
      });
    }
    const fonts = await renderedFonts(page, first.elements);
    const a11y = await accessibleNames(page);
    return {
      viewport,
      full,
      fullTruncated,
      first,
      page: whole,
      fonts,
      a11y,
      errors: errors.slice(),
    };
  } finally {
    await page.close().catch(() => {});
  }
}
