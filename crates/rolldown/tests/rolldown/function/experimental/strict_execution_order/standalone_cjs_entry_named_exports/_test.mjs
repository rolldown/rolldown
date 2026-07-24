import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const bundle = require('./dist/main.js');

assert.deepStrictEqual(bundle, { foo: 1 });
