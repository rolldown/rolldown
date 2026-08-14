// @ts-nocheck
import assert from 'node:assert';
import { value } from './dist/main';

assert.strictEqual(value, 'resolved');
