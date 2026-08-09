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
const staticRouteFile = /import\(["']\.\/([^"']*route-static[^"']*)["']\)/.exec(mainCode)?.[1];
const opaqueRouteFile = /import\(["']\.\/([^"']*route-opaque[^"']*)["']\)/.exec(mainCode)?.[1];
assert.ok(staticRouteFile && opaqueRouteFile);

assert.match(staticClosure('main.js'), /leaf-cn/);
assert.doesNotMatch(staticClosure('main.js'), /leaf-stack|cjs-a|cjs-b/);
assert.match(staticClosure(staticRouteFile), /leaf-stack/);
assert.doesNotMatch(staticClosure(staticRouteFile), /leaf-cn|cjs-a|cjs-b/);
assert.match(staticClosure(opaqueRouteFile), /leaf-cn/);
assert.match(staticClosure(opaqueRouteFile), /leaf-stack/);
assert.match(staticClosure(opaqueRouteFile), /cjs-a/);
assert.match(staticClosure(opaqueRouteFile), /cjs-b/);

globalThis.__events = [];
globalThis.__namespaceKey = 'a';
const entry = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__events, ['leaf-cn', 'main:cn']);

assert.strictEqual((await entry.loadOpaque()).value, 'a');
assert.deepStrictEqual(globalThis.__events, [
  'leaf-cn',
  'main:cn',
  'leaf-stack',
  'cjs-a',
  'cjs-b',
  'route-opaque',
]);

assert.strictEqual((await entry.loadStatic()).value, 1);
assert.deepStrictEqual(globalThis.__events, [
  'leaf-cn',
  'main:cn',
  'leaf-stack',
  'cjs-a',
  'cjs-b',
  'route-opaque',
  'route-static',
]);
