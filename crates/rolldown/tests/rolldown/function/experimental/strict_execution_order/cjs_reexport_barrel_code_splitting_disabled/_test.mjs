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

// With code splitting disabled everything shares one chunk. Wrap-all still routes the barrel per
// consumer — the carrier keeps the unrelated CJS leaf lazy behind the inlined route trigger. The
// on-demand plan sees no cross-chunk hazard in a single chunk, leaves the barrel out of its wrap
// plan, and its monolithic interop body makes the leaf eager. Both orders respect source order —
// only the eager set differs between the two plans, in wrap-all's favor.
assert.deepStrictEqual([...files.keys()], ['main.js']);
const onDemand = globalThis.__configName === 'on-demand';
if (onDemand) {
  assert.doesNotMatch(files.get('main.js'), /init_pure_barrel_cjs/);
} else {
  assert.match(files.get('main.js'), /init_pure_barrel_cjs/);
}

globalThis.__events = [];

const entry = await import('./dist/main.js');

const entryEvents = onDemand ? ['cn', 'clone-deep', 'main:a b'] : ['cn', 'main:a b'];
assert.deepStrictEqual(globalThis.__events, entryEvents);

const route = await entry.loadRoute();

assert.deepStrictEqual(route.value, { value: 1 });
assert.deepStrictEqual(globalThis.__events, [
  ...entryEvents,
  ...(onDemand ? [] : ['clone-deep']),
  'route',
]);
