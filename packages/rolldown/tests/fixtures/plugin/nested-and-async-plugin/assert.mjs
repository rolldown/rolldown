// @ts-nocheck
import assert from 'node:assert';
import { answer, foo } from './dist/main';

assert.equal(foo, 2);
assert.equal(answer, 42);
