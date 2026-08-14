import assert from 'node:assert';
import https from 'node:https';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { getDefaultExport } = require('./dist/main.js');

assert.strictEqual(getDefaultExport(), https);
