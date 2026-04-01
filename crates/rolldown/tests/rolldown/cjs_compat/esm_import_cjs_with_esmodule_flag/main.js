import assert from 'node:assert';
import * as cjs from './cjs.cjs';

// In node ESM mode ("type": "module"), __toESM ignores __esModule flag
// and .default represents the whole module.exports, not exports.default.
assert.deepStrictEqual(cjs.default, { default: 'default' });
