import assert from 'node:assert';
import { spawnSync } from 'node:child_process';
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
assert.doesNotMatch(mainClosure, /pure-leaves\/stack\.js/);
assert.match(staticClosure(routeFile), /pure-leaves\/stack\.js/);

const bareUrl = new URL('./dist/bare.js', import.meta.url).href;
const bareEntry = spawnSync(
  process.execPath,
  [
    '--input-type=module',
    '--eval',
    `globalThis.__events = []; await import(${JSON.stringify(bareUrl)}); console.log(JSON.stringify(globalThis.__events));`,
  ],
  { encoding: 'utf8' },
);
assert.deepStrictEqual(
  { status: bareEntry.status, stderr: bareEntry.stderr, stdout: bareEntry.stdout },
  { status: 0, stderr: '', stdout: '["effect-before","effect-after","bare"]\n' },
);

globalThis.__events = [];

const entry = await import('./dist/main.js');

// Both modes execute the same eager set. Flag-off hoists the generated CJS interop above the
// barrel's leaves (lazy-init transfer); strict mode pins each require to its re-export source
// position, which is the ordering this fixture family protects.
const entryEvents =
  globalThis.__configName === 'flag-off'
    ? ['effect-before', 'effect-after', 'cn', 'main:a b']
    : ['effect-before', 'cn', 'effect-after', 'main:a b'];
assert.deepStrictEqual(globalThis.__events, entryEvents);

await entry.loadRoute();

assert.deepStrictEqual(globalThis.__events, [...entryEvents, 'stack', 'route']);
