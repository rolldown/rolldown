import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), ['a', 'ta', 'a', 'helper', 'root', 't']);
