// @ts-nocheck
import assert from 'node:assert';
import { f } from './dist/main';

// A constant before the first variable must not be duplicated into the runtime path.
const m = await f('foo');
assert.strictEqual(m.default, 'foo-loaded');
