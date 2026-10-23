import assert from 'node:assert/strict';
import './dist/main.js';

assert.equal(globalThis.result, 'be');
