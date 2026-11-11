import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const dist = new URL('./dist/', import.meta.url);

assert.deepStrictEqual(
  readdirSync(dist, { recursive: true })
    .map((file) => file.replaceAll('\\', '/'))
    .filter((file) => file.endsWith('.js'))
    .sort(),
  ['rolldown-runtime.js', 'x/a.js', 'y/z/b.js'],
  'shared.js is inlined into both entries instead of becoming its own chunk',
);

const assets = readdirSync(new URL('assets/', dist));
const assetUrl = (name) =>
  new URL(`assets/${assets.find((file) => file.startsWith(`${name}-`))}`, dist).href;

// A fresh process per entry: in one process, the second entry reuses the copy of `shared.js` that
// the first one registered, and its own copy never runs.
for (const entry of ['x/a.js', 'y/z/b.js']) {
  const output = execFileSync(process.execPath, [fileURLToPath(new URL(entry, dist))], {
    encoding: 'utf8',
  });
  assert.deepStrictEqual(
    JSON.parse(output),
    [`${assetUrl('hooked')}#${entry}`, assetUrl('plain')],
    `${entry} resolves through its own copy`,
  );
}
