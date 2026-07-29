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

// An `export *` from an external module gives the barrel dynamic exports, which rejects
// consumer-local routing. The monolithic barrel couples every consumer to its complete dependency
// set: the unrelated CJS leaf and the external import both join the eager entry phase.
assert.match(staticClosure('main.js'), /clone-deep\.cjs/);

globalThis.__events = [];

const entry = await import('./dist/main.js');

// The external import's position differs between the plans (wrap-all hoists the deferred chunk's
// static imports ahead of every wrapper; on-demand keeps the eager barrel body at its source
// position), but both keep the whole monolithic barrel — external included — in the eager phase.
const entryEvents =
  globalThis.__configName === 'on-demand'
    ? ['cn', 'ext-lib', 'clone-deep', 'main:a b']
    : ['ext-lib', 'cn', 'clone-deep', 'main:a b'];
assert.deepStrictEqual(globalThis.__events, entryEvents);

const route = await entry.loadRoute();

assert.deepStrictEqual(route.value, { value: 1 });
assert.deepStrictEqual(globalThis.__events, [...entryEvents, 'route:ext']);
