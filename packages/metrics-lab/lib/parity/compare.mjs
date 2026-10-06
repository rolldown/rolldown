// Rendered-output comparison - the pure half of parity. Every function here takes
// plain data (decoded screenshots, the snapshots lib/parity/capture.mjs collects)
// and returns plain data, so each decision is unit-testable without a browser.
//
// The rule the thresholds serve: an LCP optimization must not change what the page
// renders. A visual-regression service compares runs from different machines and
// needs a percentage tolerance; here the original build and the candidate render
// back to back in ONE browser session, so the tolerance is the noise we MEASURE
// (two captures of the original under perturbed clocks and random seeds), never a
// share of the screen we guess. A guessed 0.5% lets a missing 24px icon through.

import crypto from 'node:crypto';

/** A pixel counts as changed when any channel moves by more than this (of 255). */
export const CHANNEL_TOLERANCE = 10;
/**
 * Pixels that differ between the two captures of the original grow by this radius
 * before they mask anything - the anti-aliased fringe of content that moves.
 */
export const UNSTABLE_GROW_PX = 2;
/** Fewer changed pixels than this is raster noise, not a rendering change. */
export const MIN_CHANGED_PIXELS = 20;
/**
 * A view whose unstable share exceeds this cannot be vouched for: "nothing changed"
 * would only mean "nothing changed in the part that holds still".
 */
export const UNSTABLE_LIMIT = 0.3;
/** Region clustering grid, px. */
export const CELL_PX = 8;
/** An LCP element smaller than this share of the original's is a different paint. */
export const LCP_SHRINK_LIMIT = 0.75;

/**
 * Per-pixel change mask between two RGBA images over their common area (or the
 * given bounds, so masks from different image pairs line up).
 */
export function changedPixels(a, b, tolerance = CHANNEL_TOLERANCE, bounds = null) {
  const width = bounds?.width ?? Math.min(a.width, b.width);
  const height = bounds?.height ?? Math.min(a.height, b.height);
  const mask = new Uint8Array(width * height);
  const ad = a.data;
  const bd = b.data;
  let count = 0;
  for (let y = 0; y < height; y++) {
    let ia = y * a.width * 4;
    let ib = y * b.width * 4;
    const row = y * width;
    for (let x = 0; x < width; x++, ia += 4, ib += 4) {
      if (
        Math.abs(ad[ia] - bd[ib]) > tolerance ||
        Math.abs(ad[ia + 1] - bd[ib + 1]) > tolerance ||
        Math.abs(ad[ia + 2] - bd[ib + 2]) > tolerance ||
        Math.abs(ad[ia + 3] - bd[ib + 3]) > tolerance
      ) {
        mask[row + x] = 1;
        count++;
      }
    }
  }
  return { width, height, mask, count };
}

/** Square dilation of a 0/1 mask (two running-sum passes, O(pixels)). */
export function grow({ width, height, mask }, radius) {
  if (radius <= 0) return mask.slice();
  const horizontal = new Uint8Array(mask.length);
  for (let y = 0; y < height; y++) {
    const row = y * width;
    let run = 0;
    for (let x = 0; x < Math.min(radius, width); x++) run += mask[row + x];
    for (let x = 0; x < width; x++) {
      if (x + radius < width) run += mask[row + x + radius];
      if (run > 0) horizontal[row + x] = 1;
      if (x - radius >= 0) run -= mask[row + x - radius];
    }
  }
  const out = new Uint8Array(mask.length);
  const runs = new Int32Array(width);
  const addRow = (y, sign) => {
    const row = y * width;
    for (let x = 0; x < width; x++) runs[x] += sign * horizontal[row + x];
  };
  for (let y = 0; y < Math.min(radius, height); y++) addRow(y, 1);
  for (let y = 0; y < height; y++) {
    if (y + radius < height) addRow(y + radius, 1);
    const row = y * width;
    for (let x = 0; x < width; x++) if (runs[x] > 0) out[row + x] = 1;
    if (y - radius >= 0) addRow(y - radius, -1);
  }
  return out;
}

/**
 * Connected regions of changed pixels on a CELL_PX grid. Cells up to two apart
 * join, so the words of one changed line of text (gaps between words exceed one
 * cell in a wide font) form one region. Also returns every changed cell - the
 * approval signature keys on exactly these.
 */
export function clusterRegions({ width, height, mask }, cell = CELL_PX) {
  const cols = Math.ceil(width / cell);
  const rows = Math.ceil(height / cell);
  const counts = new Uint32Array(cols * rows);
  for (let y = 0; y < height; y++) {
    const rowCell = Math.floor(y / cell) * cols;
    const row = y * width;
    for (let x = 0; x < width; x++) if (mask[row + x]) counts[rowCell + Math.floor(x / cell)]++;
  }
  const seen = new Uint8Array(counts.length);
  const queue = new Int32Array(counts.length);
  const regions = [];
  const cells = [];
  for (let start = 0; start < counts.length; start++) {
    if (!counts[start] || seen[start]) continue;
    let head = 0;
    let tail = 0;
    queue[tail++] = start;
    seen[start] = 1;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -1;
    let maxY = -1;
    let pixels = 0;
    while (head < tail) {
      const c = queue[head++];
      cells.push(c);
      const cx = c % cols;
      const cy = Math.floor(c / cols);
      pixels += counts[c];
      minX = Math.min(minX, cx);
      maxX = Math.max(maxX, cx);
      minY = Math.min(minY, cy);
      maxY = Math.max(maxY, cy);
      for (let dy = -2; dy <= 2; dy++) {
        for (let dx = -2; dx <= 2; dx++) {
          const nx = cx + dx;
          const ny = cy + dy;
          if (nx < 0 || ny < 0 || nx >= cols || ny >= rows) continue;
          const n = ny * cols + nx;
          if (counts[n] && !seen[n]) {
            seen[n] = 1;
            queue[tail++] = n;
          }
        }
      }
    }
    regions.push({
      x: minX * cell,
      y: minY * cell,
      w: Math.min(width, (maxX + 1) * cell) - minX * cell,
      h: Math.min(height, (maxY + 1) * cell) - minY * cell,
      pixels,
    });
  }
  regions.sort((a, b) => b.pixels - a.pixels);
  cells.sort((a, b) => a - b);
  return { regions, cells, cols };
}

/** Paint the declared mask rectangles opaque so masked content can never differ. */
export function paintRects(img, rects) {
  for (const r of rects) {
    const x0 = Math.max(0, Math.floor(r.x));
    const y0 = Math.max(0, Math.floor(r.y));
    const x1 = Math.min(img.width, Math.ceil(r.x + r.w));
    const y1 = Math.min(img.height, Math.ceil(r.y + r.h));
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        const i = (y * img.width + x) * 4;
        img.data[i] = 255;
        img.data[i + 1] = 0;
        img.data[i + 2] = 255;
        img.data[i + 3] = 255;
      }
    }
  }
}

/**
 * One view (viewport or full page) of one page. a1/a2 are two captures of the
 * original under different clocks and seeds; c1 is the candidate under a1's; c2 is
 * an optional re-capture of the candidate - a pixel then only counts when BOTH
 * candidate captures disagree with the original, so a one-off flake cannot open a
 * lead. Changes inside the grown unstable mask are counted but never judged.
 */
export function compareView({ a1, a2, c1, c2 = null }) {
  const width = Math.min(a1.width, a2.width, c1.width, c2?.width ?? Infinity);
  const height = Math.min(a1.height, a2.height, c1.height, c2?.height ?? Infinity);
  const bounds = { width, height };
  const unstable = changedPixels(a1, a2, CHANNEL_TOLERANCE, bounds);
  const unstableGrown = grow(unstable, UNSTABLE_GROW_PX);
  const changed = changedPixels(a1, c1, CHANNEL_TOLERANCE, bounds);
  const confirm = c2 ? changedPixels(a1, c2, CHANNEL_TOLERANCE, bounds) : null;
  const real = new Uint8Array(width * height);
  let changedCount = 0;
  let maskedCount = 0;
  for (let i = 0; i < real.length; i++) {
    if (!changed.mask[i]) continue;
    if (unstableGrown[i]) {
      maskedCount++;
      continue;
    }
    if (confirm && !confirm.mask[i]) continue;
    real[i] = 1;
    changedCount++;
  }
  const { regions, cells } = clusterRegions({ width, height, mask: real });
  const total = Math.max(1, width * height);
  const heightUnstable = a1.height !== a2.height;
  const candidateHeights = c2 ? [c1.height, c2.height] : [c1.height];
  const heightChange =
    !heightUnstable && candidateHeights.every((h) => h !== a1.height)
      ? { from: a1.height, to: c1.height }
      : null;
  return {
    width,
    height,
    changedPixels: changedCount,
    changedRatio: changedCount / total,
    maskedPixels: maskedCount,
    unstableRatio: unstable.count / total,
    heightUnstable,
    heightChange,
    regions,
    cells,
    masks: { real, unstable: unstableGrown },
  };
}

/** open = the candidate renders differently; unknown = the original itself won't hold still. */
export function viewState(view) {
  if (view.changedPixels >= MIN_CHANGED_PIXELS || view.heightChange) return 'open';
  if (view.unstableRatio > UNSTABLE_LIMIT || view.heightUnstable) return 'unknown';
  return 'clear';
}

/**
 * Diagnostic image: the candidate faded to gray, changed pixels red, unstable
 * (unjudged) pixels blue, rows past the original's height pink.
 */
export function renderDiffImage(candidate, view) {
  const { width, height } = candidate;
  const out = new Uint8Array(width * height * 4);
  const src = candidate.data;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      const gray = 0.3 * src[i] + 0.59 * src[i + 1] + 0.11 * src[i + 2];
      const faded = 255 - (255 - gray) * 0.35;
      let r = faded;
      let g = faded;
      let b = faded;
      if (x < view.width && y < view.height) {
        const m = y * view.width + x;
        if (view.masks.real[m]) {
          r = 230;
          g = 0;
          b = 60;
        } else if (view.masks.unstable[m]) {
          r = faded * 0.75;
          g = faded * 0.85;
          b = 255;
        }
      } else if (y >= view.height) {
        g = faded * 0.7;
        b = faded * 0.8;
        r = 255;
      }
      out[i] = r;
      out[i + 1] = g;
      out[i + 2] = b;
      out[i + 3] = 255;
    }
  }
  return { width, height, data: out };
}

// --- content ------------------------------------------------------------------

function counts(list) {
  const m = new Map();
  for (const item of list) m.set(item, (m.get(item) ?? 0) + 1);
  return m;
}

/** Items whose multiplicity differs between two captures of the original. */
export function unstableItems(a, b) {
  const ca = counts(a);
  const cb = counts(b);
  const out = new Set();
  for (const [item, n] of ca) if (cb.get(item) !== n) out.add(item);
  for (const item of cb.keys()) if (!ca.has(item)) out.add(item);
  return out;
}

/** Order-insensitive diff: what the candidate lost and gained, unstable items skipped. */
export function multisetDiff(before, after, unstable = new Set()) {
  const cb = counts(before);
  const ca = counts(after);
  const fmt = (item, n) => (n > 1 ? `${item} (x${n})` : item);
  const missing = [];
  const added = [];
  for (const [item, n] of cb) {
    const d = n - (ca.get(item) ?? 0);
    if (d > 0 && !unstable.has(item)) missing.push(fmt(item, d));
  }
  for (const [item, n] of ca) {
    const d = n - (cb.get(item) ?? 0);
    if (d > 0 && !unstable.has(item)) added.push(fmt(item, d));
  }
  return { missing, added };
}

/** innerText -> comparable lines (whitespace collapsed, blanks dropped). */
export function textLines(text) {
  return String(text ?? '')
    .split('\n')
    .map((line) => line.replace(/\s+/g, ' ').trim())
    .filter(Boolean);
}

/**
 * Runtime error text comparable across builds: no origin/port, no line:column
 * (every bundle change moves them), hashed asset names reduced to their stem,
 * first line only (stacks carry minified frame names that differ per build).
 */
export function normalizeRuntimeError(text) {
  return String(text ?? '')
    .split('\n')[0]
    .replace(/https?:\/\/(?:127\.0\.0\.1|localhost)(?::\d+)?/g, '')
    .replace(/([\w.]+)-[\w-]{8,}(\.[a-z0-9]+)\b/gi, '$1$2')
    .replace(/:\d+(?::\d+)?\b/g, '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 240);
}

// --- elements -----------------------------------------------------------------

const sameRect = (a, b) =>
  Math.abs(a.x - b.x) <= 1 &&
  Math.abs(a.y - b.y) <= 1 &&
  Math.abs(a.w - b.w) <= 1 &&
  Math.abs(a.h - b.h) <= 1;

const isVisible = (e) =>
  e.style.visibility !== 'hidden' && e.style.opacity !== '0' && e.style.display !== 'none';

export function styleChanges(a, b) {
  const out = [];
  for (const prop of Object.keys(a)) {
    if (a[prop] !== b[prop]) out.push({ prop, from: a[prop], to: b[prop] });
  }
  return out;
}

/** Element paths whose rect, style or text differ between two captures of the original. */
export function unstableElements(a1, a2) {
  const other = new Map(a2.map((e) => [e.path, e]));
  const out = new Set();
  for (const e of a1) {
    const o = other.get(e.path);
    if (
      !o ||
      e.text !== o.text ||
      !sameRect(e.rect, o.rect) ||
      styleChanges(e.style, o.style).length
    ) {
      out.add(e.path);
    }
    other.delete(e.path);
  }
  for (const path of other.keys()) out.add(path);
  return out;
}

/**
 * Match candidate elements to the original's: by structural path first, then -
 * for a DOM that gained or lost a wrapper - by tag + own text when that pair is
 * unique on both sides.
 */
export function matchElements(anchorEls, currentEls) {
  const byPath = new Map(currentEls.map((e) => [e.path, e]));
  const byText = new Map();
  for (const e of currentEls) {
    if (!e.text) continue;
    const key = `${e.tag}|${e.text}`;
    byText.set(key, byText.has(key) ? null : e);
  }
  const anchorTextCount = counts(anchorEls.filter((e) => e.text).map((e) => `${e.tag}|${e.text}`));
  const pairs = new Map();
  for (const a of anchorEls) {
    let c = byPath.get(a.path);
    const key = `${a.tag}|${a.text}`;
    if (!c && a.text && anchorTextCount.get(key) === 1) c = byText.get(key) ?? undefined;
    if (c) pairs.set(a.path, c);
  }
  return pairs;
}

/**
 * Explanations, not verdicts: which matched elements changed computed style or
 * moved, which visible ones vanished or appeared. Pixels and text decide; this
 * says why.
 */
export function compareElements(anchorEls, currentEls, unstable = new Set()) {
  const pairs = matchElements(anchorEls, currentEls);
  const matched = new Set(Array.from(pairs.values(), (c) => c.path));
  const changed = [];
  const moved = [];
  const missing = [];
  for (const a of anchorEls) {
    if (unstable.has(a.path)) continue;
    const c = pairs.get(a.path);
    if (!c) {
      if (isVisible(a)) missing.push(a);
      continue;
    }
    if (!isVisible(a) && !isVisible(c)) continue;
    const changes = styleChanges(a.style, c.style);
    if (changes.length) changed.push({ element: a, current: c, changes });
    else if (!sameRect(a.rect, c.rect)) moved.push({ element: a, current: c });
  }
  const added = currentEls.filter(
    (c) => !matched.has(c.path) && !unstable.has(c.path) && isVisible(c),
  );
  return { changed, moved, missing, added };
}

/** The elements that best explain a changed region (tightest overlap first). */
export function elementsAt(region, elements, limit = 2) {
  const regionArea = region.w * region.h;
  const scored = [];
  for (const e of elements) {
    const overlapW =
      Math.min(region.x + region.w, e.rect.x + e.rect.w) - Math.max(region.x, e.rect.x);
    const overlapH =
      Math.min(region.y + region.h, e.rect.y + e.rect.h) - Math.max(region.y, e.rect.y);
    if (overlapW <= 0 || overlapH <= 0) continue;
    const overlap = overlapW * overlapH;
    scored.push({ e, score: overlap / (regionArea + e.rect.w * e.rect.h - overlap) });
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, limit).map((s) => s.e);
}

// --- LCP element --------------------------------------------------------------

const assetPath = (url) => String(url ?? '').replace(/^https?:\/\/[^/]+/, '');

/**
 * The LCP gain is only real when LCP still timestamps the same content. Three ways
 * it stops doing so, each a cheaper paint than the original's:
 * - a different element became the LCP (the hero was hidden, shrunk or replaced);
 * - the same element, much smaller;
 * - a placeholder: LCP fired on content the element later replaced (a low-quality
 *   image swapped for the real one, skeleton text swapped for copy, a skeleton node
 *   removed for the real hero) - LCP emits no new entry for a same-size swap, so
 *   the timestamp belongs to the placeholder. Only counts when the original did
 *   not already do it.
 * An image URL change alone is not one (a format conversion renames the file).
 */
export function compareLcp(anchor, current) {
  if (!anchor?.tag || !current?.tag) return { state: 'unknown', anchor, current };
  const placeholder = (d) =>
    d.detached === true ||
    (d.url && d.finalUrl && assetPath(d.url) !== assetPath(d.finalUrl)) ||
    (!d.url && d.finalText != null && d.text !== d.finalText);
  const shownText = (d) => d.finalText ?? d.text ?? '';
  const shownUrl = (d) => assetPath(d.finalUrl ?? d.url);
  // Path first; a restructured DOM falls back to what the element shows. Two
  // textless images are only the same element when they show the same file.
  const sameElement =
    (anchor.path && anchor.path === current.path) ||
    (anchor.tag === current.tag &&
      ((shownText(anchor) !== '' && shownText(anchor) === shownText(current)) ||
        (shownUrl(anchor) !== '' && shownUrl(anchor) === shownUrl(current))));
  if (!sameElement) {
    // A page whose LCP element already flips between loads can't convict a change.
    const flips = (d) => d.runs != null && d.of != null && d.runs < d.of;
    return { state: flips(anchor) || flips(current) ? 'unstable' : 'changed', anchor, current };
  }
  if (placeholder(current) && !placeholder(anchor))
    return { state: 'placeholder', anchor, current };
  if (anchor.size > 0 && current.size < anchor.size * LCP_SHRINK_LIMIT) {
    return { state: 'shrank', anchor, current };
  }
  return { state: 'same', anchor, current };
}

/** "img.hero 1280x600 (/assets/hero.jpg)" */
export function describeLcp(d) {
  if (!d?.tag) return 'n/a';
  const box = d.rect ? ` ${d.rect.w}x${d.rect.h}` : '';
  const what = d.url ? ` (${assetPath(d.url)})` : d.text ? ` "${d.text.slice(0, 40)}"` : '';
  return `${d.label ?? d.tag}${box}${what}`;
}

export function lcpReason(lcp) {
  const from = describeLcp(lcp.anchor);
  const to = describeLcp(lcp.current);
  if (lcp.state === 'changed') return `LCP element changed: ${from} -> ${to}`;
  if (lcp.state === 'shrank') return `LCP element shrank: ${from} -> ${to}`;
  const replacement = lcp.current.detached
    ? 'content that replaced it (the element was removed)'
    : lcp.current.url
      ? assetPath(lcp.current.finalUrl)
      : `"${lcp.current.finalText}"`;
  return `LCP fired on a placeholder: ${to}, later replaced by ${replacement}`;
}

// --- one page -----------------------------------------------------------------

const pct = (ratio) => `${(ratio * 100).toFixed(ratio < 0.001 ? 3 : 1)}%`;
const area = (e) => e.rect.w * e.rect.h;
const byAreaDesc = (a, b) => area(b) - area(a);
const brief = (e) => (e.text ? `${e.label} "${e.text.slice(0, 40)}"` : e.label);

const intersectDiffs = (a, b) => ({
  missing: a.missing.filter((item) => b.missing.includes(item)),
  added: a.added.filter((item) => b.added.includes(item)),
});

/** A page that crosses the viewport height in some captures only: heights, no pixels. */
function heightOnlyView(a1h, a2h, c1h) {
  return {
    width: 0,
    height: 0,
    changedPixels: 0,
    changedRatio: 0,
    maskedPixels: 0,
    unstableRatio: 0,
    heightUnstable: a1h !== a2h,
    heightChange: a1h === a2h && c1h !== a1h ? { from: a1h, to: c1h } : null,
    regions: [],
    cells: [],
    masks: null,
  };
}

/**
 * Everything parity knows about one page, from two captures of the original (a1
 * under condition A, a2 under B) and the candidate under A (c1, plus the
 * confirmation re-capture c2 when there is one). Captures carry decoded images.
 *
 * Judged (any one opens the page): pixels and height of the first screen and the
 * full page, visible text, accessible names, where the page ended up, new runtime
 * errors, and - for the measured page - the LCP element. Explanatory only: computed
 * styles, rendered fonts, images, loaded font faces, and the elements under each
 * changed region. A legitimate deferral changes none of the judged facts; it may
 * well change explanatory ones (a wrapper element, a renamed image file).
 */
export function comparePage({ a1, a2, c1, c2 = null, lcp = null }) {
  const caps = [a1, a2, c1, c2].filter(Boolean);
  const viewportMasks = caps.flatMap((c) => c.first.masks);
  const fullMasks = caps.flatMap((c) => c.page.masks);
  for (const c of new Set(caps)) {
    paintRects(c.images.viewport, viewportMasks);
    if (c.images.full) paintRects(c.images.full, fullMasks);
  }

  const views = {
    viewport: compareView({
      a1: a1.images.viewport,
      a2: a2.images.viewport,
      c1: c1.images.viewport,
      c2: c2?.images.viewport ?? null,
    }),
  };
  const fullShots = caps.map((c) => c.images.full);
  if (fullShots.every(Boolean)) {
    views.full = compareView({
      a1: a1.images.full,
      a2: a2.images.full,
      c1: c1.images.full,
      c2: c2?.images.full ?? null,
    });
  } else if (fullShots.some(Boolean)) {
    views.full = heightOnlyView(
      a1.page.contentHeight,
      a2.page.contentHeight,
      c1.page.contentHeight,
    );
  }

  const maskedLines = new Set(caps.flatMap((c) => textLines(c.page.maskedText.join('\n'))));
  const lines = (c) => textLines(c.page.text).filter((line) => !maskedLines.has(line));
  const unstableText = unstableItems(lines(a1), lines(a2));
  let text = multisetDiff(lines(a1), lines(c1), unstableText);
  if (c2) text = intersectDiffs(text, multisetDiff(lines(a1), lines(c2), unstableText));

  let a11y = { missing: [], added: [] };
  if (caps.every((c) => c.a11y)) {
    const unstableNames = unstableItems(a1.a11y, a2.a11y);
    a11y = multisetDiff(a1.a11y, c1.a11y, unstableNames);
    if (c2) a11y = intersectDiffs(a11y, multisetDiff(a1.a11y, c2.a11y, unstableNames));
  }

  const known = new Set([...a1.errors, ...a2.errors].map((e) => normalizeRuntimeError(e)));
  const fresh = (c) =>
    new Set(c.errors.map((e) => normalizeRuntimeError(e)).filter((e) => e && !known.has(e)));
  let newErrors = fresh(c1);
  if (c2) {
    const again = fresh(c2);
    newErrors = new Set(Array.from(newErrors).filter((e) => again.has(e)));
  }
  const runtime = { added: Array.from(newErrors) };

  const location =
    a1.page.location === a2.page.location && c1.page.location !== a1.page.location
      ? { from: a1.page.location, to: c1.page.location }
      : null;

  // --- explanations
  const firstPairs = matchElements(a1.first.elements, c1.first.elements);
  const pagePairs = matchElements(a1.page.elements, c1.page.elements);
  const unstableFirst = unstableElements(a1.first.elements, a2.first.elements);
  const unstablePage = unstableElements(a1.page.elements, a2.page.elements);
  const explain = (region, anchorEls, currentEls, pairs, unstable) => {
    const matchedCurrent = new Set(Array.from(pairs.values(), (c) => c.path));
    const items = [];
    const seen = new Set();
    for (const a of elementsAt(region, anchorEls)) {
      const c = pairs.get(a.path);
      if (c) seen.add(c.path);
      if (unstable.has(a.path)) continue;
      const item = { label: a.label, text: a.text.slice(0, 60) };
      if (!c) item.gone = true;
      else {
        const changes = styleChanges(a.style, c.style);
        if (changes.length) item.changes = changes.slice(0, 4);
        const from = a1.fonts[a.path];
        const to = c1.fonts[c.path];
        if (from && to && from !== to) item.font = { from, to };
        if (!sameRect(a.rect, c.rect)) item.moved = { from: a.rect, to: c.rect };
      }
      items.push(item);
    }
    for (const c of elementsAt(region, currentEls)) {
      if (seen.has(c.path)) continue;
      items.push({
        label: c.label,
        text: c.text.slice(0, 60),
        ...(matchedCurrent.has(c.path) ? { movedIn: true } : { added: true }),
      });
    }
    // Name what changed, strongest first. A box that only moved (a container
    // resized by its changed child) is named only when nothing stronger is
    // there, and an unchanged container only when nothing at all explains it.
    const strong = items.filter((i) => i.gone || i.added || i.changes || i.font);
    const weak = items.filter((i) => !strong.includes(i) && (i.movedIn || i.moved));
    if (strong.length) return strong.slice(0, 3);
    return weak.length ? weak.slice(0, 2) : items.slice(0, 1);
  };
  for (const region of views.viewport.regions.slice(0, 4)) {
    region.elements = explain(
      region,
      a1.first.elements,
      c1.first.elements,
      firstPairs,
      unstableFirst,
    );
  }
  for (const region of views.full?.regions.slice(0, 4) ?? []) {
    region.elements = explain(region, a1.page.elements, c1.page.elements, pagePairs, unstablePage);
  }

  const fonts = [];
  for (const [anchorPath, c] of firstPairs) {
    const from = a1.fonts[anchorPath];
    const to = c1.fonts[c.path];
    if (!from || !to || from === to || a2.fonts[anchorPath] !== from) continue;
    fonts.push({ label: c.label, text: c.text.slice(0, 60), from, to });
  }
  const images = multisetDiff(
    a1.page.images,
    c1.page.images,
    unstableItems(a1.page.images, a2.page.images),
  );
  const loaded = (c) =>
    c.page.faces.filter((f) => f.endsWith(': loaded')).map((f) => f.slice(0, -': loaded'.length));
  const faces = multisetDiff(loaded(a1), loaded(c1), unstableItems(loaded(a1), loaded(a2)));
  const elements = compareElements(a1.page.elements, c1.page.elements, unstablePage);
  const styles = {
    changed: elements.changed
      .sort((x, y) => byAreaDesc(x.element, y.element))
      .slice(0, 8)
      .map((d) => ({
        label: d.element.label,
        text: d.element.text.slice(0, 60),
        changes: d.changes.slice(0, 4),
      })),
    changedCount: elements.changed.length,
    movedCount: elements.moved.length,
    missing: elements.missing.sort(byAreaDesc).slice(0, 5).map(brief),
    missingCount: elements.missing.length,
    added: elements.added.sort(byAreaDesc).slice(0, 5).map(brief),
    addedCount: elements.added.length,
  };

  // --- verdict
  const reasons = [];
  const notes = [];
  for (const [name, view] of Object.entries(views)) {
    view.state = viewState(view);
    const where = name === 'viewport' ? 'first screen' : 'full page';
    if (view.heightChange) {
      reasons.push(
        `${where}: page height ${view.heightChange.from}px -> ${view.heightChange.to}px`,
      );
    }
    if (view.changedPixels >= MIN_CHANGED_PIXELS) {
      const n = view.regions.length;
      reasons.push(
        `${where}: ${pct(view.changedRatio)} of pixels changed (${n} region${n === 1 ? '' : 's'})`,
      );
    }
    if (view.state === 'unknown') {
      notes.push(
        view.heightUnstable
          ? `${where}: the original's page height changes between loads - not judged`
          : `${where}: ${pct(view.unstableRatio)} of the original's pixels change between two loads - too unstable to vouch for (declare masks for the moving parts)`,
      );
    }
    if (view.maskedPixels > 0) {
      notes.push(
        `${where}: ${view.maskedPixels} changed pixel(s) lie where the original itself varies between loads - not judged`,
      );
    }
  }
  if (location) {
    reasons.push(`the page ended at ${location.to} (the original ended at ${location.from})`);
  }
  if (text.missing.length || text.added.length) {
    reasons.push(`text: ${text.missing.length} line(s) missing, ${text.added.length} new`);
  }
  if (a11y.missing.length || a11y.added.length) {
    reasons.push(`accessible names: ${a11y.missing.length} missing, ${a11y.added.length} new`);
  }
  if (runtime.added.length) reasons.push(`${runtime.added.length} new runtime error(s)`);
  if (lcp && ['changed', 'shrank', 'placeholder'].includes(lcp.state)) reasons.push(lcpReason(lcp));
  if (lcp?.state === 'unstable') {
    notes.push(
      `the LCP element varies between loads (${describeLcp(lcp.anchor)} / ${describeLcp(lcp.current)}) - not compared`,
    );
  }
  if (a1.fullTruncated)
    notes.push(`full page compared down to its first ${a1.images.full.height}px`);
  if (a1.page.truncated || c1.page.truncated) {
    notes.push('element list truncated - style explanations cover the first elements only');
  }
  for (const selector of new Set(caps.flatMap((c) => c.first.badMasks))) {
    notes.push(`mask selector does not parse, ignored: ${selector}`);
  }
  const state = reasons.length
    ? 'open'
    : Object.values(views).some((v) => v.state === 'unknown')
      ? 'unknown'
      : 'clear';
  return {
    state,
    reasons,
    notes,
    views,
    text,
    a11y,
    runtime,
    location,
    lcp,
    fonts,
    images,
    faces,
    styles,
  };
}

// --- signature ----------------------------------------------------------------

/**
 * What a human approval is keyed on: the exact changed picture (the candidate's
 * pixels inside every changed cell of each view) plus every judged difference.
 * Any further change to the page - a new region, a different wrong color, one
 * more missing line - produces a new signature, so an old approval stops applying.
 */
export function pageSignature(result, candidateImages) {
  const hash = crypto.createHash('sha256');
  for (const name of ['viewport', 'full']) {
    const view = result.views[name];
    const img = candidateImages[name];
    if (!view || !img) continue;
    hash.update(`${name}:${view.cells.length}:${JSON.stringify(view.heightChange)}`);
    const cols = Math.ceil(view.width / CELL_PX);
    for (const cell of view.cells) {
      const x0 = (cell % cols) * CELL_PX;
      const y0 = Math.floor(cell / cols) * CELL_PX;
      hash.update(`${cell};`);
      for (let y = y0; y < Math.min(y0 + CELL_PX, view.height); y++) {
        const start = (y * img.width + x0) * 4;
        hash.update(img.data.subarray(start, start + Math.min(CELL_PX, view.width - x0) * 4));
      }
    }
  }
  hash.update(
    JSON.stringify({
      reasons: result.reasons,
      text: result.text,
      a11y: result.a11y,
      runtime: result.runtime,
      location: result.location,
      lcp: result.lcp ? { state: result.lcp.state, current: result.lcp.current } : null,
    }),
  );
  return hash.digest('hex').slice(0, 16);
}
