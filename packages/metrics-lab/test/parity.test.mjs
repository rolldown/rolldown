// Rendered-output parity. The parts worth pinning are the ones that decide whether a
// faster build COUNTS: a broken page must open a lead (a missed break is the failure
// this whole mode exists to prevent), and a page that merely moves on its own - or a
// legitimate deferral that renders the same - must not (a false alarm teaches the
// loop to ignore the signal).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import zlib from 'node:zlib';

import { decodePng, encodePng } from '../lib/parity/png.mjs';
import {
  MIN_CHANGED_PIXELS,
  changedPixels,
  clusterRegions,
  compareLcp,
  comparePage,
  compareView,
  elementsAt,
  grow,
  matchElements,
  multisetDiff,
  normalizeRuntimeError,
  pageSignature,
  textLines,
  unstableItems,
  viewState,
} from '../lib/parity/compare.mjs';
import { discoverPages, hashDist, normalizePagePath } from '../lib/parity/anchor.mjs';
import { startServer } from '../lib/serve.mjs';
import { lcpElementOf } from '../lib/measure.mjs';

// --- fixtures -----------------------------------------------------------------

function image(width, height, fill = [255, 255, 255, 255]) {
  const data = new Uint8Array(width * height * 4);
  for (let i = 0; i < data.length; i += 4) data.set(fill, i);
  return { width, height, data };
}

function paint(img, { x, y, w, h }, color) {
  for (let row = y; row < y + h; row++) {
    for (let col = x; col < x + w; col++) img.data.set(color, (row * img.width + col) * 4);
  }
  return img;
}

const BLUE = [37, 99, 235, 255];
const GRAY = [239, 239, 239, 255];
const byText = (a, b) => a.localeCompare(b);

const button = (style = {}) => ({
  path: 'body:1>main:1>button:1',
  tag: 'button',
  label: 'button.cta',
  rect: { x: 20, y: 20, w: 30, h: 10 },
  style: { 'background-color': 'rgb(37, 99, 235)', opacity: '1', visibility: 'visible', ...style },
  text: 'Start free trial',
});

function capture({
  viewport = image(80, 60),
  full = null,
  text = 'Ship faster\nStart free trial',
  a11y = ['button "Start free trial"'],
  errors = [],
  elements = [button()],
  fonts = {},
  location = '/',
} = {}) {
  return {
    images: { viewport, full },
    fullTruncated: false,
    first: { elements, masks: [], badMasks: [] },
    page: {
      elements,
      text,
      images: [],
      faces: [],
      masks: [],
      maskedText: [],
      location,
      contentHeight: full ? full.height : viewport.height,
      truncated: false,
    },
    fonts,
    a11y,
    errors,
  };
}

// --- PNG ----------------------------------------------------------------------

test('PNG encode/decode round-trips RGBA pixels exactly', () => {
  const img = paint(image(13, 7, [10, 20, 30, 255]), { x: 2, y: 1, w: 5, h: 3 }, [200, 0, 90, 128]);
  const back = decodePng(encodePng(img));
  assert.equal(back.width, 13);
  assert.equal(back.height, 7);
  assert.deepEqual(Array.from(back.data), Array.from(img.data));
});

test('PNG decoding undoes every scanline filter (Chrome mixes them per row)', () => {
  // 2x5 RGB image, one row per filter type 0..4, built by hand.
  const width = 2;
  const rows = [
    [10, 20, 30, 40, 50, 60],
    [11, 22, 33, 44, 55, 66],
    [12, 24, 36, 48, 60, 72],
    [13, 26, 39, 52, 65, 78],
    [14, 28, 42, 56, 70, 84],
  ];
  const bpp = 3;
  const paeth = (a, b, c) => {
    const p = a + b - c;
    const pa = Math.abs(p - a);
    const pb = Math.abs(p - b);
    const pc = Math.abs(p - c);
    return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
  };
  const raw = [];
  rows.forEach((row, y) => {
    const prev = y ? rows[y - 1] : Array.from({ length: row.length }, () => 0);
    raw.push(y);
    row.forEach((v, i) => {
      const left = i >= bpp ? row[i - bpp] : 0;
      const upLeft = i >= bpp ? prev[i - bpp] : 0;
      const predictor = [
        0,
        left,
        prev[i],
        Math.floor((left + prev[i]) / 2),
        paeth(left, prev[i], upLeft),
      ][y];
      raw.push((v - predictor + 256) % 256);
    });
  });
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    return Buffer.concat([len, Buffer.from(type, 'latin1'), data, Buffer.alloc(4)]); // CRC unchecked
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(rows.length, 4);
  ihdr[8] = 8;
  ihdr[9] = 2; // RGB
  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(Buffer.from(raw))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
  const { data } = decodePng(png);
  rows.forEach((row, y) => {
    for (let x = 0; x < width; x++) {
      const o = (y * width + x) * 4;
      assert.deepEqual(Array.from(data.subarray(o, o + 4)), [...row.slice(x * 3, x * 3 + 3), 255]);
    }
  });
});

// --- pixels -------------------------------------------------------------------

test('a channel move within tolerance is not a change; a visible one is', () => {
  const a = image(4, 4, [100, 100, 100, 255]);
  const b = paint(
    image(4, 4, [100, 100, 100, 255]),
    { x: 0, y: 0, w: 2, h: 1 },
    [108, 100, 100, 255],
  );
  assert.equal(changedPixels(a, b).count, 0);
  paint(b, { x: 0, y: 0, w: 2, h: 1 }, [117, 100, 100, 255]); // #333 -> #444 sized step
  assert.equal(changedPixels(a, b).count, 2);
});

test('grow dilates a mask by its radius and keeps it inside the image', () => {
  const mask = new Uint8Array(25);
  mask[12] = 1; // center of 5x5
  const grown = grow({ width: 5, height: 5, mask }, 1);
  assert.equal(
    grown.reduce((s, v) => s + v, 0),
    9,
  );
  assert.equal(grown[0], 0);
  const corner = new Uint8Array(25);
  corner[0] = 1;
  assert.equal(
    grow({ width: 5, height: 5, mask: corner }, 2).reduce((s, v) => s + v, 0),
    9,
  );
});

test('changed pixels cluster into one region per changed area', () => {
  const width = 64;
  const height = 64;
  const mask = new Uint8Array(width * height);
  const fill = (x0, y0, w, h) => {
    for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) mask[y * width + x] = 1;
  };
  fill(2, 2, 6, 4);
  fill(48, 40, 10, 10);
  const { regions } = clusterRegions({ width, height, mask });
  assert.equal(regions.length, 2);
  assert.deepEqual(
    regions.map((r) => [r.x, r.y, r.pixels]),
    [
      [48, 40, 100],
      [0, 0, 24],
    ],
  );
});

test('a change where the original itself varies is masked, a change elsewhere is not', () => {
  const a1 = image(80, 60);
  const a2 = paint(image(80, 60), { x: 0, y: 0, w: 20, h: 10 }, [0, 0, 0, 255]); // a clock
  const c1 = paint(image(80, 60), { x: 0, y: 0, w: 20, h: 10 }, [50, 50, 50, 255]);
  let view = compareView({ a1, a2, c1 });
  assert.equal(view.changedPixels, 0);
  assert.ok(view.maskedPixels > 0);
  assert.equal(viewState(view), 'clear');

  paint(c1, { x: 40, y: 30, w: 10, h: 10 }, BLUE);
  view = compareView({ a1, a2, c1 });
  assert.equal(view.changedPixels, 100);
  assert.equal(viewState(view), 'open');
});

test('a difference only one candidate capture shows is a flake, not a change', () => {
  const a1 = image(80, 60);
  const a2 = image(80, 60);
  const c1 = paint(image(80, 60), { x: 10, y: 10, w: 10, h: 10 }, BLUE);
  const c2 = image(80, 60);
  assert.equal(compareView({ a1, a2, c1, c2 }).changedPixels, 0);
});

test('a page height change opens the view; a height the original cannot hold is unknown', () => {
  const view = compareView({ a1: image(40, 100), a2: image(40, 100), c1: image(40, 80) });
  assert.deepEqual(view.heightChange, { from: 100, to: 80 });
  assert.equal(viewState(view), 'open');
  const unsteady = compareView({ a1: image(40, 100), a2: image(40, 90), c1: image(40, 80) });
  assert.equal(unsteady.heightChange, null);
  assert.equal(viewState(unsteady), 'unknown');
});

test('an original that varies over most of the view cannot vouch for anything', () => {
  const a1 = image(40, 40);
  const a2 = paint(image(40, 40), { x: 0, y: 0, w: 40, h: 20 }, [0, 0, 0, 255]);
  assert.equal(viewState(compareView({ a1, a2, c1: image(40, 40) })), 'unknown');
});

// --- content ------------------------------------------------------------------

test('content diffs are order-insensitive and skip what the original varies', () => {
  const before = textLines('Hero\n  Start   free trial \n\nLucky 12');
  const again = textLines('Hero\nStart free trial\nLucky 87');
  const after = textLines('Lucky 40\nHero');
  const unstable = unstableItems(before, again);
  assert.deepEqual(Array.from(unstable).toSorted(byText), ['Lucky 12', 'Lucky 87']);
  assert.deepEqual(multisetDiff(before, after, unstable), {
    missing: ['Start free trial'],
    added: ['Lucky 40'],
  });
});

test('runtime errors compare across builds: no origins, positions or chunk hashes', () => {
  const a = normalizeRuntimeError(
    'uncaught TypeError: x is not a function\n    at Xe (http://127.0.0.1:53211/assets/index-BxQ3k9aZ.js:1:2345)',
  );
  const b = normalizeRuntimeError('uncaught TypeError: x is not a function\n    at q (other)');
  assert.equal(a, b);
  assert.equal(
    normalizeRuntimeError('HTTP 404 http://127.0.0.1:4100/assets/chunk-D41xYz9q.js'),
    normalizeRuntimeError('HTTP 404 http://127.0.0.1:5200/assets/chunk-Hh72kQ0p.js'),
  );
});

// --- elements -----------------------------------------------------------------

test('elements match by path, and by unique tag + text across a new wrapper', () => {
  const anchor = [button(), { ...button(), path: 'body:1>h1:1', tag: 'h1', text: 'Hero' }];
  const wrapped = anchor.map((e) => ({ ...e, path: e.path.replace('body:1>', 'body:1>div:1>') }));
  const pairs = matchElements(anchor, wrapped);
  assert.equal(pairs.size, 2);
  assert.equal(pairs.get('body:1>h1:1').path, 'body:1>div:1>h1:1');
});

test('a changed region is explained by the element that fits it best', () => {
  const big = { label: 'main', rect: { x: 0, y: 0, w: 1280, h: 900 } };
  const cta = { label: 'button.cta', rect: { x: 32, y: 512, w: 164, h: 45 } };
  assert.deepEqual(
    elementsAt({ x: 32, y: 512, w: 168, h: 48 }, [big, cta]).map((e) => e.label),
    ['button.cta', 'main'],
  );
});

// --- LCP element --------------------------------------------------------------

const hero = {
  tag: 'h1',
  path: 'body:1>div:1>section:1>h1:1',
  label: 'h1#hero-title',
  text: 'Ship faster',
  finalText: 'Ship faster',
  url: '',
  size: 49_000,
  runs: 3,
  of: 3,
};

test('LCP on the same element is the same paint; another element is not', () => {
  assert.equal(compareLcp(hero, { ...hero }).state, 'same');
  const subtitle = {
    ...hero,
    tag: 'p',
    path: 'body:1>div:1>section:1>p:1',
    text: 'Handcrafted insights',
    finalText: 'Handcrafted insights',
    size: 23_000,
  };
  assert.equal(compareLcp(hero, subtitle).state, 'changed');
  assert.equal(compareLcp(hero, { ...hero, size: 20_000 }).state, 'shrank');
});

test('LCP fired on a placeholder: swapped image, skeleton text, removed node', () => {
  const img = {
    ...hero,
    tag: 'img',
    text: '',
    finalText: '',
    url: 'http://x/hero.jpg',
    finalUrl: '/hero.jpg',
  };
  assert.equal(compareLcp(img, { ...img }).state, 'same');
  assert.equal(compareLcp(img, { ...img, url: 'http://x/hero-blur.jpg' }).state, 'placeholder');
  assert.equal(compareLcp(hero, { ...hero, text: 'Loading...' }).state, 'placeholder');
  assert.equal(compareLcp(hero, { ...hero, detached: true }).state, 'placeholder');
  // a renamed file (format conversion) is not a placeholder on its own
  const webp = { ...img, url: 'http://x/hero.webp', finalUrl: '/hero.webp' };
  assert.equal(compareLcp(img, webp).state, 'same');
});

test('an LCP element that already flips between loads cannot convict a change', () => {
  const other = {
    ...hero,
    tag: 'p',
    path: 'body:1>p:1',
    text: 'x',
    finalText: 'x',
    runs: 2,
    of: 3,
  };
  assert.equal(compareLcp(hero, other).state, 'unstable');
});

test('the LCP element across runs is the majority one, with how often it won', () => {
  const sample = (d) => ({ lcpElement: d });
  const p = { ...hero, path: 'body:1>p:1', tag: 'p' };
  const winner = lcpElementOf([sample(hero), sample(p), sample(hero), sample(null)]);
  assert.equal(winner.path, hero.path);
  assert.equal(winner.runs, 2);
  assert.equal(winner.of, 4);
});

// --- one page -----------------------------------------------------------------

test('the same render with a different DOM shape is clear', () => {
  const wrapped = [{ ...button(), path: 'body:1>div:1>main:1>button:1' }];
  const result = comparePage({ a1: capture(), a2: capture(), c1: capture({ elements: wrapped }) });
  assert.equal(result.state, 'clear');
  assert.deepEqual(result.reasons, []);
});

test('a style that stopped applying opens the page and names the element and property', () => {
  const broken = paint(image(80, 60), { x: 20, y: 20, w: 30, h: 10 }, GRAY);
  const a = () => paint(image(80, 60), { x: 20, y: 20, w: 30, h: 10 }, BLUE);
  const result = comparePage({
    a1: capture({ viewport: a() }),
    a2: capture({ viewport: a() }),
    c1: capture({
      viewport: broken,
      elements: [button({ 'background-color': 'rgb(239, 239, 239)' })],
    }),
  });
  assert.equal(result.state, 'open');
  assert.match(result.reasons[0], /first screen: .* of pixels changed \(1 region\)/);
  const [region] = result.views.viewport.regions;
  assert.equal(region.pixels, 300);
  assert.equal(region.elements[0].label, 'button.cta');
  assert.deepEqual(region.elements[0].changes, [
    { prop: 'background-color', from: 'rgb(37, 99, 235)', to: 'rgb(239, 239, 239)' },
  ]);
});

test('lost text, lost accessible names and new runtime errors each open a page', () => {
  const lostText = comparePage({
    a1: capture(),
    a2: capture(),
    c1: capture({ text: 'Ship faster' }),
  });
  assert.deepEqual(lostText.text.missing, ['Start free trial']);
  assert.equal(lostText.state, 'open');

  const lostName = comparePage({ a1: capture(), a2: capture(), c1: capture({ a11y: [] }) });
  assert.deepEqual(lostName.a11y.missing, ['button "Start free trial"']);

  const known = 'HTTP 404 http://127.0.0.1:1/api/user';
  const errors = comparePage({
    a1: capture({ errors: [known] }),
    a2: capture({ errors: [known] }),
    c1: capture({ errors: [known, 'uncaught TypeError: boom'] }),
  });
  assert.deepEqual(errors.runtime.added, ['uncaught TypeError: boom']);
  assert.equal(errors.state, 'open');
});

test('an LCP element that changed opens the measured page', () => {
  const result = comparePage({
    a1: capture(),
    a2: capture(),
    c1: capture(),
    lcp: compareLcp(hero, { ...hero, path: 'body:1>p:1', tag: 'p', text: 'x', finalText: 'x' }),
  });
  assert.equal(result.state, 'open');
  assert.match(result.reasons.at(-1), /^LCP element changed: h1#hero-title/);
});

test('an approval keys on the exact changed picture', () => {
  const run = (color) => {
    const result = comparePage({
      a1: capture(),
      a2: capture(),
      c1: capture({ viewport: paint(image(80, 60), { x: 20, y: 20, w: 30, h: 10 }, color) }),
    });
    return pageSignature(result, {
      viewport: paint(image(80, 60), { x: 20, y: 20, w: 30, h: 10 }, color),
    });
  };
  assert.ok(MIN_CHANGED_PIXELS <= 300);
  assert.equal(run(GRAY), run(GRAY));
  assert.notEqual(run(GRAY), run([200, 30, 30, 255]));
});

// --- the original build ---------------------------------------------------------

function tempDist(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'metrics-lab-parity-'));
  for (const [rel, content] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
    fs.writeFileSync(path.join(dir, rel), content);
  }
  return dir;
}

test('entry pages are the HTML files that load bundled output', () => {
  const dir = tempDist({
    'index.html': '<!doctype html><div id="app"></div>',
    'about/index.html': '<script type="module" src="/assets/about.js"></script>',
    'docs.html': '<link href="/assets/docs.css" rel="stylesheet">',
    'stats.html': '<script>window.data = {}</script><link rel="icon" href="/favicon.ico">',
    'cdn.html': '<script src="https://cdn.example.com/x.js"></script>',
  });
  const { pages, skipped } = discoverPages(dir);
  assert.deepEqual(pages, ['/', '/about/', '/docs.html']);
  assert.deepEqual(skipped.toSorted(byText), ['cdn.html', 'stats.html']);
});

test('the build hash follows content, not file order or timestamps', () => {
  const a = tempDist({ 'index.html': 'x', 'assets/a.js': '1' });
  const b = tempDist({ 'assets/a.js': '1', 'index.html': 'x' });
  assert.equal(hashDist(a), hashDist(b));
  fs.writeFileSync(path.join(b, 'assets/a.js'), '2');
  assert.notEqual(hashDist(a), hashDist(b));
  fs.mkdirSync(path.join(a, 'state'));
  fs.writeFileSync(path.join(a, 'state/x.json'), '{}');
  assert.equal(
    hashDist(a, { exclude: [path.join(a, 'state')] }),
    hashDist(tempDist({ 'index.html': 'x', 'assets/a.js': '1' })),
  );
});

test('--pages entries must be paths on the app origin', () => {
  assert.equal(normalizePagePath(' /pricing '), '/pricing');
  assert.throws(() => normalizePagePath('pricing'), /starting with "\/"/);
  assert.throws(() => normalizePagePath('//evil.example/x'), /starting with "\/"/);
});

// --- serving --------------------------------------------------------------------

function get(origin, pathname, headers = {}) {
  return new Promise((resolve, reject) => {
    http
      .get(`${origin}${pathname}`, { headers }, (res) => {
        let body = '';
        res.on('data', (chunk) => (body += chunk));
        res.on('end', () => resolve({ status: res.statusCode, body }));
      })
      .on('error', reject);
  });
}

test('navigations get directory indexes and the SPA fallback; subresources still 404', async () => {
  const dir = tempDist({ 'index.html': 'root', 'about/index.html': 'about', 'docs.html': 'docs' });
  const server = await startServer(dir);
  const nav = { 'sec-fetch-mode': 'navigate' };
  try {
    assert.equal((await get(server.origin, '/about/', nav)).body, 'about');
    assert.equal((await get(server.origin, '/docs', nav)).body, 'docs');
    assert.equal((await get(server.origin, '/pricing', nav)).body, 'root');
    assert.equal((await get(server.origin, '/api/user')).status, 404);
    assert.equal((await get(server.origin, '/missing.js', nav)).status, 404);
  } finally {
    await server.close();
  }
});

test('a function root swaps what one origin serves', async () => {
  const one = tempDist({ 'index.html': 'one' });
  const two = tempDist({ 'index.html': 'two' });
  let root = one;
  const server = await startServer(() => root);
  try {
    assert.equal((await get(server.origin, '/')).body, 'one');
    root = two;
    assert.equal((await get(server.origin, '/')).body, 'two');
  } finally {
    await server.close();
  }
});
