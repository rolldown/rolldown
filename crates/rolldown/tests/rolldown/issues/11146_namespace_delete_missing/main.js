import assert from 'node:assert';
import { env } from './dynamic.js';

assert.strictEqual(delete env.missing, true);
assert.strictEqual(delete env?.missing, true);
