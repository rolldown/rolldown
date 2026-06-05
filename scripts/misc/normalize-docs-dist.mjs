// Normalize a built docs `dist` directory in place so two builds of the *same* source compare
// equal, despite non-reproducible output from some vitepress plugins. Used only to gate the docs
// deploy by diffing the built site between two commits — it never touches what we actually deploy.
//
// It removes the build noise that is not reproducible:
//   1. `llms.txt` / `llms-full.txt` — vitepress-plugin-llms emits entries in unstable order.
//   2. group-icons CSS — the `[data-title]` rules come out in unstable order *and* (in CI) with
//      unstable content (the label-collection races), which also changes the CSS content hash.
//      We drop those rules from the CSS and de-hash the CSS filename. The rest of the CSS is
//      deterministic. (A change that only adds/removes a code-group icon won't trigger a deploy,
//      but the code content that drives it still shows up in the HTML.)
//   3. references to the hashed CSS filename in every text file.
// A real content change still shows up: HTML/JS are compared as-is, and a real (non group-icons)
// CSS change yields different chunks.
import fs from 'node:fs';
import path from 'node:path';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: node normalize-docs-dist.mjs <dist-dir>');
  process.exit(1);
}

const CSS_HASH_RE = /\.[A-Za-z0-9_-]{8}\.css/g;
const TEXT_RE = /\.(html?|js|mjs|cjs|css|txt|xml|json|svg|map)$/;

// 1. drop volatile generated aggregations
for (const name of ['llms.txt', 'llms-full.txt']) {
  fs.rmSync(path.join(dir, name), { force: true });
}

// 2. drop the group-icons rules from the CSS, canonicalize the rest, strip the hash from the name
const assetsDir = path.join(dir, 'assets');
if (fs.existsSync(assetsDir)) {
  for (const name of fs.readdirSync(assetsDir)) {
    if (!name.endsWith('.css')) continue;
    const p = path.join(assetsDir, name);
    const css = fs.readFileSync(p, 'utf8');
    // Split into `}`-delimited chunks (output need not be valid CSS), drop any chunk whose
    // selectors target group-icons (`[data-title=...]`), sort the remaining selectors, sort chunks.
    const chunks = css
      .split('}')
      .filter((chunk) => !chunk.includes('[data-title'))
      .map((chunk) => {
        const i = chunk.indexOf('{');
        if (i === -1) return chunk;
        const selectors = chunk
          .slice(0, i)
          .split(',')
          .map((s) => s.trim())
          .sort()
          .join(',');
        return `${selectors}{${chunk.slice(i + 1)}`;
      });
    chunks.sort();
    fs.writeFileSync(p, chunks.join('}'));
    const stripped = name.replace(/\.[A-Za-z0-9_-]{8}\.css$/, '.css');
    if (stripped !== name) fs.renameSync(p, path.join(assetsDir, stripped));
  }
}

// 3. strip hashed CSS references from every text file
function walk(d) {
  for (const entry of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, entry.name);
    if (entry.isDirectory()) walk(p);
    else if (TEXT_RE.test(entry.name)) {
      const content = fs.readFileSync(p, 'utf8');
      const next = content.replace(CSS_HASH_RE, '.css');
      if (next !== content) fs.writeFileSync(p, next);
    }
  }
}
walk(dir);
