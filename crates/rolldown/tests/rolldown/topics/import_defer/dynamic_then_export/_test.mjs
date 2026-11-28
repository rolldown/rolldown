import assert from 'node:assert/strict';
import { eager, load } from './dist/main.js';

globalThis.events = [];
const ns = await load();
assert.equal(ns, await load());
assert.equal(ns.then, undefined);
assert.deepEqual(globalThis.events, []);
assert.equal(ns.x, 1);
assert.equal(await eager(), 'ordinary');
assert.deepEqual(globalThis.events, ['target', 'then']);
