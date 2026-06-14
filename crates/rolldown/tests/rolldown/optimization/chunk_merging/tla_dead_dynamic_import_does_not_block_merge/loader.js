import assert from 'node:assert';
import { main } from './main.js';

export const loaded = await import('./route.js');

assert.strictEqual(main, 'main:shared');
assert.strictEqual(loaded.route, 'main:shared:route');
