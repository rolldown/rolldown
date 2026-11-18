import assert from 'node:assert';
import './first.js';
import 'data:text/javascript,globalThis.order.push("second")';

assert.deepStrictEqual(globalThis.order, ['first', 'second']);
