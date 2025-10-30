import assert from 'node:assert';
import * as ns from './dist/main.js';

assert.strictEqual(ns.unused, undefined);
