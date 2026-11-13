import assert from 'node:assert';
import { LEVEL, SIZE } from './ui/index.js';

assert.strictEqual(SIZE, 'md');
assert.strictEqual(LEVEL, 1);
assert.strictEqual(globalThis.styleLoaded, true);
assert.strictEqual(globalThis.themeApplied, true);
