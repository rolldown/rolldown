import assert from 'node:assert/strict';
import path from 'node:path';
import { load, loadPage } from './dist/main.js';

// `route.js` reaches `export * from 'node:path'` through a barrel. The build moves that `export *`
// to the chunk, so the entry cannot use an extracted namespace. The debug assertion runs after
// that step, and it must skip the entry too.
const route = await load();
assert.equal(route.sep, path.sep);
assert.equal(route.value, 7);
assert.equal((await loadPage()).get().value, 7);
