import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const load = (file) => (file.endsWith('.cjs') ? require(file) : import(pathToFileURL(file)));
const [appFile, publicFile, mode] = process.argv.slice(2);
const entry = await load(publicFile);
const app = await load(appFile);
if (mode === 'internal') {
  const ns = await app.loadSelection();
  assert.deepEqual(Object.keys(ns).sort(), ['kind', 'next', 'selected']);
  assert.equal(ns, app.selection, 'static and dynamic imports must return the same namespace');
  assert.equal(ns.next, app.advance);
}
assert.equal(entry.kind, 'selection');
assert.ok(Object.hasOwn(entry, 'next'), 'the public entry is missing the re-export "next"');
assert.equal(entry.next, app.advance, 'the public entry must export the original function as next');
assert.equal(entry.selected, app.current);

const previous = entry.selected;
entry.next();
assert.notEqual(app.current, previous);
assert.equal(entry.selected, app.current, 'the public re-export must follow reassignment');
if (mode === 'internal') assert.equal(app.selection.selected, app.current);
