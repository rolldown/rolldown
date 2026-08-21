import assert from 'node:assert';
import { readdir, readFile } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const distDir = fileURLToPath(new URL('./dist/', import.meta.url));
const files = new Map(
  await Promise.all(
    (await readdir(distDir))
      .filter((name) => name.endsWith('.js'))
      .map(async (name) => [name, await readFile(join(distDir, name), 'utf8')]),
  ),
);

function staticClosure(root) {
  const seen = new Set();
  const pending = [root];
  while (pending.length > 0) {
    const name = pending.pop();
    if (seen.has(name)) continue;
    seen.add(name);
    const code = files.get(name);
    assert.ok(code, `missing emitted chunk ${name}`);
    for (const match of code.matchAll(
      /(?:^|\n)import(?:[^"']*?from\s*)?\s*["']\.\/([^"']+)["']/g,
    )) {
      pending.push(basename(join(dirname(name), match[1])));
    }
  }
  return [...seen].map((name) => files.get(name)).join('\n');
}

// A TLA-tainted barrel rejects consumer-local routing, so the monolithic barrel init keeps the
// unrelated CJS leaf in the eager entry closure.
assert.match(staticClosure('main.js'), /clone-deep\.cjs/);

globalThis.__events = [];

const entry = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.__events, ['cn', 'clone-deep', 'main:a b']);

const route = await entry.loadRoute();

assert.deepStrictEqual(route.value, { value: 1 });
assert.deepStrictEqual(globalThis.__events, ['cn', 'clone-deep', 'main:a b', 'route']);
