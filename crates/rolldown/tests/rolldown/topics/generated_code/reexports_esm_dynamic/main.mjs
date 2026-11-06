import assert from 'node:assert';
import * as ns from './reexports.mjs';

assert.equal(Object.prototype.toString.call(ns), '[object Module]');
