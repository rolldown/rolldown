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

const mainCode = files.get('main.js');
const routeAFile = /import\(["']\.\/([^"']*route-a[^"']*)["']\)/.exec(mainCode)?.[1];
const routeBFile = /import\(["']\.\/([^"']*route-b[^"']*)["']\)/.exec(mainCode)?.[1];
assert.ok(routeAFile && routeBFile, 'main must retain both dynamic route boundaries');

assert.doesNotMatch(staticClosure('main.js'), /cjs-[ab]/);
assert.match(staticClosure(routeAFile), /cjs-a/);
assert.doesNotMatch(staticClosure(routeAFile), /cjs-b/);
assert.match(staticClosure(routeBFile), /cjs-b/);
assert.doesNotMatch(staticClosure(routeBFile), /cjs-a/);

globalThis.__events = [];
const entry = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__events, ['cn', 'main:cn']);
assert.strictEqual((await entry.loadA()).value, 'a');
assert.deepStrictEqual(globalThis.__events, ['cn', 'main:cn', 'cjs-a', 'route-a']);
assert.strictEqual((await entry.loadB()).value, 'b');
assert.deepStrictEqual(globalThis.__events, [
  'cn',
  'main:cn',
  'cjs-a',
  'route-a',
  'cjs-b',
  'route-b',
]);
