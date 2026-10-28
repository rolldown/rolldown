import assert from 'node:assert';
import './css.js';

assert.strictEqual(globalThis.cssResult, 'a:1');
