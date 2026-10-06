// The original build parity compares every later build against (the "anchor").
// Saved once - by the first scan / measure / parity of a target - as a full copy
// of the build directory, not as screenshots: each check re-renders it next to the
// candidate in the same browser session, so a Chrome update, a newly installed
// font or machine load can never surface as a difference.
//
// Unlike the perf baseline, the optimize loop never re-pins it. Compared to the
// step before it, a broken step looks clean and becomes the reference for the next
// one; compared to the original, every step answers the only question that
// matters - does this build still render what the original rendered?

import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const SKIP_DIRS = new Set(['node_modules', '.git']);
/** Discovered entry pages beyond this are listed, not pinned (pin them with --pages). */
export const DISCOVER_LIMIT = 12;

export function parityPaths(targetDir) {
  const dir = path.join(targetDir, 'parity');
  return {
    dir,
    anchorDir: path.join(dir, 'anchor'),
    anchorDist: path.join(dir, 'anchor', 'dist'),
    manifest: path.join(dir, 'anchor', 'manifest.json'),
    report: path.join(dir, 'report.json'),
    approvals: path.join(dir, 'approvals.json'),
    pages: path.join(dir, 'pages'),
  };
}

const excluded = (file, exclude) =>
  exclude.some((e) => file === e || file.startsWith(e + path.sep));

/** Every file under dir as a sorted list of '/'-separated relative paths. */
function listFiles(dir, exclude = []) {
  const out = [];
  const visit = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (excluded(full, exclude)) continue;
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) visit(full);
      } else if (entry.isFile()) {
        out.push(path.relative(dir, full).split(path.sep).join('/'));
      }
    }
  };
  visit(dir);
  // Code-unit order, not locale order: the hash must not depend on the machine.
  return out.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
}

/**
 * Content hash of a build: every file's path and bytes. Two builds with the same
 * hash render the same by construction, which lets parity skip the candidate
 * captures entirely (first scan, or every change reverted).
 */
export function hashDist(dir, { exclude = [] } = {}) {
  const hash = crypto.createHash('sha256');
  for (const rel of listFiles(dir, exclude)) {
    hash.update(`${rel}\0`);
    hash.update(
      crypto
        .createHash('sha256')
        .update(fs.readFileSync(path.join(dir, rel)))
        .digest(),
    );
  }
  return hash.digest('hex');
}

/** Does this HTML file load bundled output (a local script, stylesheet or modulepreload)? */
function loadsBundle(html) {
  const local = (url) => Boolean(url) && !/^(?:[a-z]+:|\/\/)/i.test(url);
  for (const tag of html.match(/<(?:script|link)\b[^>]*>/gi) ?? []) {
    const attr = (name) => tag.match(new RegExp(`\\b${name}\\s*=\\s*["']?([^"'\\s>]+)`, 'i'))?.[1];
    if (/^<script/i.test(tag) && local(attr('src'))) return true;
    if (
      /^<link/i.test(tag) &&
      /stylesheet|modulepreload/i.test(attr('rel') ?? '') &&
      local(attr('href'))
    ) {
      return true;
    }
  }
  return false;
}

/**
 * The build's entry pages: index.html plus every other HTML file that loads
 * bundled output. Self-contained HTML (a bundle-analyzer stats.html, a static
 * 404) is skipped - its content follows the bundle, not the app, and would differ
 * after every legitimate change.
 */
export function discoverPages(distDir, { exclude = [] } = {}) {
  const pages = [];
  const skipped = [];
  for (const rel of listFiles(distDir, exclude)) {
    if (!rel.endsWith('.html')) continue;
    const page =
      rel === 'index.html'
        ? '/'
        : rel.endsWith('/index.html')
          ? `/${rel.slice(0, -'index.html'.length)}`
          : `/${rel}`;
    if (page === '/' || loadsBundle(fs.readFileSync(path.join(distDir, rel), 'utf8'))) {
      pages.push(page);
    } else {
      skipped.push(rel);
    }
  }
  pages.sort((a, b) => (a === '/' ? -1 : b === '/' ? 1 : a.localeCompare(b)));
  return { pages: pages.slice(0, DISCOVER_LIMIT), more: pages.slice(DISCOVER_LIMIT), skipped };
}

/** Normalize a --pages entry to a server path, or throw with the expected shape. */
export function normalizePagePath(input) {
  const page = String(input).trim();
  if (!page.startsWith('/') || page.startsWith('//')) {
    throw new Error(
      `--pages entries are paths on the app's own origin, starting with "/" (got "${page}")`,
    );
  }
  return page;
}

/** The app's commit and whether its tree had uncommitted changes (build output excluded). */
export function gitState(appRoot, distDir) {
  try {
    const run = (args) =>
      execFileSync('git', ['-C', appRoot, ...args], {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'ignore'],
        timeout: 10_000,
      }).trim();
    const sha = run(['rev-parse', '--short', 'HEAD']);
    const relDist = path.relative(appRoot, distDir);
    const pathspec = relDist && !relDist.startsWith('..') ? ['.', `:(exclude)${relDist}`] : ['.'];
    const dirty = run(['status', '--porcelain', '--', ...pathspec]).length > 0;
    return { sha, dirty };
  } catch {
    return null;
  }
}

/**
 * Copy the build into the parity state and return its manifest (the caller writes
 * it). `exclude` keeps the lab's own state out when it lives inside the build dir.
 */
export function saveAnchor({
  dist,
  paths,
  pages,
  masks = [],
  git = null,
  late = false,
  exclude = [],
}) {
  fs.rmSync(paths.anchorDir, { recursive: true, force: true });
  fs.mkdirSync(paths.anchorDist, { recursive: true });
  fs.cpSync(dist, paths.anchorDist, {
    recursive: true,
    filter: (src) => !excluded(src, exclude) && !SKIP_DIRS.has(path.basename(src)),
  });
  const now = Date.now();
  return {
    schemaVersion: 1,
    createdAtMs: now,
    dist,
    distHash: hashDist(paths.anchorDist),
    git,
    late,
    pages: pages.map((page) => ({ path: page, addedAtMs: now })),
    masks: masks.map((selector) => ({ selector, addedAtMs: now })),
    replaced: [],
    lcp: null,
  };
}
