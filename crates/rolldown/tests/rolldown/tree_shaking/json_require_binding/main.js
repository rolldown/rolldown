import assert from 'node:assert/strict';
import './eval.js';
import imported from './imported.json';
import ordinary from './ordinary.js';

const config = require('./config.json');
assert.strictEqual(config.mode, 'prod');

const extraArgument = require('./extra-argument.json', 'ignored');
assert.strictEqual(extraArgument.mode, 'extra argument');

const array = require('./array.json');
assert.strictEqual(array.length, 2);

const primitive = require('./primitive.json');
assert.strictEqual(primitive.length, 9);

const defaultKey = require('./default-key.json');
assert.strictEqual(defaultKey.default, 'preserved');

const mutated = require('./mutated.json');
mutated.mode = 'dev';
assert.strictEqual(mutated.mode, 'dev');

const requiredImported = require('./imported.json');
imported.mode = 'imported mutation';
assert.strictEqual(requiredImported.mode, 'imported mutation');

const escaped = require('./escaped.json');
Reflect.set(escaped.valueOf(), 'mode', 'escaped mutation');
assert.strictEqual(escaped.mode, 'escaped mutation');

Object.defineProperty(Object.prototype, 'absent', {
  configurable: true,
  get() {
    this.mode = 'prototype mutation';
    return undefined;
  },
});
const missing = require('./missing.json');
assert.strictEqual(missing.mode, 'fallback');
assert.strictEqual(missing.absent, undefined);
const missingReader = require('./missing.json');
assert.strictEqual(missingReader.mode, 'prototype mutation');
delete Object.prototype.absent;

const requiredOrdinary = require('./ordinary.json');
ordinary.mode = 'ordinary mutation';
assert.strictEqual(requiredOrdinary.mode, 'ordinary mutation');

const lazy = require('./lazy.json');
function readLazy() {
  return lazy.mode;
}
assert.strictEqual(readLazy(), 'lazy');
