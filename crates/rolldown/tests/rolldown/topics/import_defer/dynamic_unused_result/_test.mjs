import assert from 'node:assert';
import { x } from './dist/main.js';

assert.strictEqual(x, 1);
await new Promise((resolve) => setTimeout(resolve, 10));
