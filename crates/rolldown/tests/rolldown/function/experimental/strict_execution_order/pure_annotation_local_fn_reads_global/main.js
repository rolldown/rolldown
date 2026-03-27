import assert from 'node:assert';
import './setup.js';
import value from './dep.js';

assert.strictEqual(value, 'foo');
