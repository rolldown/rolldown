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

const mainClosure = staticClosure('main.js');
const routeFile = /import\(["']\.\/([^"']*route[^"']*)["']\)/.exec(mainClosure)?.[1];
assert.ok(routeFile, 'main must retain the dynamic route boundary');
assert.doesNotMatch(mainClosure, /effectful-clone/);
assert.match(staticClosure(routeFile), /effectful-clone/);
assert.match(staticClosure(routeFile), /nested-effectful-clone/);

globalThis.__events = [];

const entry = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.__events, ['cn', 'ancestor', 'main:cn:nested-cn']);
assert.deepStrictEqual((await entry.loadRoute()).value, [{ value: 1 }, { value: 2 }]);
// Both modes run the same lazy set on the route. Flag-off hoists the generated CJS interop of
// the nested barrel above the outer one (lazy-init transfer); strict mode keeps the re-export
// source order across both barrel hops.
assert.deepStrictEqual(globalThis.__events, [
  'cn',
  'ancestor',
  'main:cn:nested-cn',
  ...(globalThis.__configName === 'flag-off'
    ? ['nested-effectful-clone', 'effectful-clone']
    : ['effectful-clone', 'nested-effectful-clone']),
  'route',
]);
