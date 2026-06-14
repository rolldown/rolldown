import assert from 'node:assert';
import { s } from './shared.js';

// Top-level await of a dynamic import whose target statically imports the same
// `shared` module that `main` statically imports. The optimizer must not fold
// `shared` into `main`'s chunk, because that would make `route` statically
// depend on `main` while `main` awaits `route` — an awaited dependency cycle.
const route = await import('./route.js');

assert.strictEqual(s, 'shared');
assert.strictEqual(route.fromRoute, 'shared->route');
