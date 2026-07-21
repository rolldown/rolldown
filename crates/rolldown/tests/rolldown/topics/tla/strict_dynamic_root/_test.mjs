import assert from 'node:assert';
import { loaded } from './dist/main.js';

assert.strictEqual(await loaded, 'tla');
