// Normalize a built docs `dist` directory in place so two builds of the *same* source compare
// equal, despite non-reproducible output from some vitepress plugins. Used only to gate the docs
// deploy by diffing the built site between two commits — it never touches what we actually deploy.
//
// It removes three sources of build noise:
//   1. `llms.txt` / `llms-full.txt` — vitepress-plugin-llms emits entries in unstable order.
//   2. group-icons CSS — the `[data-title]` selectors (and the rules) come out in unstable order,
//      which also changes the CSS content hash; canonicalize by sorting, and de-hash the filename.
//   3. references to the hashed CSS filename in every text file.
// A real content change still shows up: HTML/JS are compared as-is, and a real CSS change yields
// different canonicalized chunks.
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

// 2. canonicalize CSS content and strip the hash from the filename
const assetsDir = path.join(dir, 'assets');
if (fs.existsSync(assetsDir)) {
  for (const name of fs.readdirSync(assetsDir)) {
    if (!name.endsWith('.css')) continue;
    const p = path.join(assetsDir, name);
    const css = fs.readFileSync(p, 'utf8');
    // Split into `}`-delimited chunks, sort the comma-separated selectors before each `{`, then
    // sort the chunks. Output need not be valid CSS — it only has to canonicalize ordering.
    const chunks = css.split('}').map((chunk) => {
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
