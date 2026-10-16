import assert from 'node:assert';
import fs from 'node:fs';
import { register } from 'node:module';

const distUrl = new URL('./dist/', import.meta.url).href;
const { imports } = JSON.parse(fs.readFileSync(new URL('importmap.json', distUrl), 'utf8'));
const hashedChunks = fs
  .readdirSync(new URL(distUrl))
  .filter((file) => file.endsWith('.js') && !['a.js', 'b.js', 'c.js'].includes(file));

assert.deepStrictEqual(
  Object.values(imports).sort(),
  hashedChunks.map((file) => '/' + file).sort(),
  'every hashed chunk has its own import map entry',
);

// Node has no import map support, so resolve chunk imports through `importmap.json` the way a browser would.
const hooks = `
let distUrl, imports;
export function initialize(data) {
  ({ distUrl, imports } = data);
}
export function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith('./') && context.parentURL?.startsWith(distUrl)) {
    const mapped = imports['/' + new URL(specifier, context.parentURL).href.slice(distUrl.length)];
    if (mapped) return { url: distUrl + mapped.slice(1), shortCircuit: true };
  }
  return nextResolve(specifier, context);
}
`;
register('data:text/javascript,' + encodeURIComponent(hooks), { data: { distUrl, imports } });

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

assert.deepStrictEqual(globalThis.events, [
  'a shared:foo shared:baz',
  'b shared:foo shared:bar',
  'c shared:bar shared:baz',
]);
