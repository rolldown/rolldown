import assert from 'node:assert';
import { cssFns } from './css.js';

assert.strictEqual(cssFns.setOpacity('blue', 0.2), 'blue:0.2');
assert.strictEqual(globalThis.colorLoaded, 1);
