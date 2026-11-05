import assert from 'node:assert';
import { result } from './dist/main.js';

assert.deepStrictEqual(result, ['the error from throws', true]);
