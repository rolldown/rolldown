// Parity runner: for each pinned page, render the original build and the candidate
// back to back in ONE browser session, from ONE origin (the server's root swaps
// between captures), compare, and write the evidence images.
//
// Capture order per page: original (A), candidate (A), original (B). Interleaving
// keeps drift within the session (a slow third-party font early on) from landing on
// one side only; the second original, under a moved clock and another random seed,
// maps what the original itself does not render the same twice.

import fs from 'node:fs';
import path from 'node:path';

import { startServer } from '../serve.mjs';
import { CONDITION_A, CONDITION_B, capturePage } from './capture.mjs';
import { MIN_CHANGED_PIXELS, comparePage, pageSignature, renderDiffImage } from './compare.mjs';
import { decodePng, encodePng } from './png.mjs';

export function pageSlug(pagePath) {
  return pagePath.replace(/[^a-z0-9.]+/gi, '-').replace(/^-+|-+$/g, '') || 'root';
}

const decode = (capture) => ({
  ...capture,
  images: {
    viewport: decodePng(capture.viewport),
    full: capture.full ? decodePng(capture.full) : null,
  },
});

function failedPage(pagePath, state, reason, lcp) {
  return {
    page: pagePath,
    state,
    reasons: state === 'open' ? [reason] : [],
    notes: state === 'open' ? [] : [reason],
    views: {},
    text: { missing: [], added: [] },
    a11y: { missing: [], added: [] },
    runtime: { added: [] },
    location: null,
    lcp,
    fonts: [],
    images: { missing: [], added: [] },
    faces: { missing: [], added: [] },
    styles: null,
  };
}

function writeEvidence(dir, { a1, c1, result, identical }) {
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  const files = {};
  const put = (name, bytes) => {
    const file = path.join(dir, `${name}.png`);
    fs.writeFileSync(file, bytes);
    files[name] = file;
  };
  put('anchor', a1.viewport);
  if (a1.full) put('anchor-full', a1.full);
  if (!identical) {
    put('current', c1.viewport);
    if (c1.full) put('current-full', c1.full);
  }
  // A diff image for every view that changed, and for every view too unstable to
  // judge - its blue areas are where masks belong.
  for (const [name, view] of Object.entries(result.views)) {
    if (!view.masks || (view.changedPixels < MIN_CHANGED_PIXELS && view.state !== 'unknown')) {
      continue;
    }
    const img = name === 'viewport' ? c1.images.viewport : c1.images.full;
    put(name === 'viewport' ? 'diff' : 'diff-full', encodePng(renderDiffImage(img, view)));
  }
  return files;
}

/**
 * Compare every page; `identical` (same dist hash as the original) skips the
 * candidate captures - the original's two captures still run, so a page too
 * unstable to judge is reported on the very first scan, before any change.
 */
export async function runParity({
  cdp,
  anchorDist,
  candidateDist,
  pages,
  masks,
  outDir,
  identical,
  lcpByPage = {},
  log = () => {},
}) {
  let root = anchorDist;
  const server = await startServer(() => root);
  const capture = async (dist, pagePath, condition) => {
    root = dist;
    try {
      return decode(await capturePage(cdp, { origin: server.origin, pagePath, condition, masks }));
    } catch (err) {
      return { error: err.message };
    }
  };
  const results = [];
  const fail = (pagePath, state, reason, lcp) => {
    const result = failedPage(pagePath, state, reason, lcp);
    result.signature = pageSignature(result, {});
    results.push(result);
  };
  try {
    for (const pagePath of pages) {
      const lcp = lcpByPage[pagePath] ?? null;
      log(`parity ${pagePath}: original build...`);
      const a1 = await capture(anchorDist, pagePath, CONDITION_A);
      let c1 = a1;
      if (!identical) {
        log(`parity ${pagePath}: this build...`);
        c1 = await capture(candidateDist, pagePath, CONDITION_A);
      }
      log(`parity ${pagePath}: original build again (moved clock, new seed)...`);
      const a2 = await capture(anchorDist, pagePath, CONDITION_B);
      if (a1.error || a2.error) {
        fail(
          pagePath,
          'unknown',
          `the original build did not render this page: ${a1.error ?? a2.error}`,
          lcp,
        );
        continue;
      }
      if (c1.error) {
        fail(pagePath, 'open', `this build did not render the page: ${c1.error}`, lcp);
        continue;
      }
      let result = comparePage({ a1, a2, c1, lcp });
      // A finding must reproduce: a second capture of this build has to disagree
      // with the original in the same pixels / lines / errors before it is reported.
      if (!identical && result.state === 'open') {
        log(`parity ${pagePath}: confirming with a second capture of this build...`);
        const c2 = await capture(candidateDist, pagePath, CONDITION_A);
        if (!c2.error) result = comparePage({ a1, a2, c1, c2, lcp });
      }
      result.page = pagePath;
      result.signature = pageSignature(result, c1.images);
      result.files = writeEvidence(path.join(outDir, pageSlug(pagePath)), {
        a1,
        c1,
        result,
        identical,
      });
      for (const view of Object.values(result.views)) {
        view.changedCells = view.cells.length;
        delete view.masks;
        delete view.cells;
      }
      results.push(result);
    }
  } finally {
    await server.close();
  }
  return results;
}
