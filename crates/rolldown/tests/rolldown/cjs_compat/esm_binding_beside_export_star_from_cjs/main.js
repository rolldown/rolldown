import assert from 'node:assert/strict';
import { config } from './lib.js';
import { config as mjsConfig } from './lib.mjs';
import * as ns from './reexport.js';

assert.strictEqual(config.version, '2.0.0');
assert.strictEqual(mjsConfig.version, '2.0.0');
assert.strictEqual(ns.y.foo, 'esm');
