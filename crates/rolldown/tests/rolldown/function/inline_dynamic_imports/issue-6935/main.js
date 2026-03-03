import assert from 'node:assert';
import { testFunc } from './mod';

assert.strictEqual(testFunc(), 1);
