import assert from 'node:assert';
import { used } from './lib/index.js';

assert.strictEqual(used(), 'used');
